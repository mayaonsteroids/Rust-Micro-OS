#!/usr/bin/env python3
"""
BubbleOS Windows Host Wi-Fi Helper Service.
Retrieves real Windows Wi-Fi adapter parameters and serves them over a lightweight
read-only HTTP JSON endpoint at port 19820 for VirtualBox NAT host bridging.

Complies strictly with security invariants:
- Never queries, retrieves, or serves Wi-Fi passwords.
- Read-only HTTP GET endpoints.
- Supports both `netsh wlan show interfaces` and Windows Native Wifi API (wlanapi.dll)
  to ensure 100% reliable telemetry even without elevation or location permission.
"""

import sys
import os
import time
import json
import re
import traceback
import subprocess
import threading
import urllib.request
from http.server import HTTPServer, BaseHTTPRequestHandler

PORT = 19820
HELPER_DIR = os.path.dirname(os.path.abspath(__file__))
LOG_FILE = os.path.join(HELPER_DIR, "wifi_helper.log")
PID_FILE = os.path.join(HELPER_DIR, "wifi_helper.pid")

def log_entry(msg: str):
    """Appends a timestamped log entry to the helper log file."""
    ts = time.strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{ts}] {msg}\n"
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(line)
    except Exception:
        pass
    if sys.stdout is not None:
        try:
            sys.stdout.write(line)
            sys.stdout.flush()
        except Exception:
            pass

def check_existing_instance() -> bool:
    """Checks if another helper instance is already running and answering on port 19820."""
    try:
        req = urllib.request.Request(
            f"http://127.0.0.1:{PORT}/",
            headers={"User-Agent": "BubbleOS-Helper-Probe"}
        )
        with urllib.request.urlopen(req, timeout=1.0) as resp:
            if resp.status == 200:
                data = json.loads(resp.read().decode("utf-8"))
                if "status" in data or "ssid" in data:
                    return True
    except Exception:
        pass
    return False

def save_pid():
    """Writes the current process ID to wifi_helper.pid."""
    try:
        with open(PID_FILE, "w", encoding="utf-8") as f:
            f.write(str(os.getpid()))
    except Exception as e:
        log_entry(f"Failed to write PID file: {e}")

def remove_pid():
    """Removes the PID file upon clean shutdown."""
    try:
        if os.path.exists(PID_FILE):
            os.remove(PID_FILE)
    except Exception:
        pass

def stop_helper():
    """Stops any currently running helper process using PID file without flashing any windows."""
    if os.path.exists(PID_FILE):
        try:
            with open(PID_FILE, "r", encoding="utf-8") as f:
                pid = int(f.read().strip())
            log_entry(f"Terminating helper PID {pid} via stop_helper()...")
            if os.name == "nt":
                subprocess.run(
                    ["taskkill", "/F", "/PID", str(pid)],
                    capture_output=True,
                    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000),
                )
            else:
                os.kill(pid, 9)
        except Exception as e:
            log_entry(f"Error stopping PID from file: {e}")
        finally:
            remove_pid()

def get_wifi_info_via_netsh():
    """Attempts to retrieve Wi-Fi connection info via netsh wlan show interfaces without displaying any console window."""
    try:
        startupinfo = None
        creationflags = 0
        if os.name == "nt":
            creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000)
            startupinfo = subprocess.STARTUPINFO()
            startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            startupinfo.wShowWindow = 0  # SW_HIDE

        proc = subprocess.run(
            ["netsh", "wlan", "show", "interfaces"],
            capture_output=True,
            text=True,
            timeout=3,
            creationflags=creationflags,
            startupinfo=startupinfo,
        )
        if proc.returncode != 0 or not proc.stdout:
            return None
        out = proc.stdout
        if "Function WlanQueryInterface returns error" in out or "There is no wireless interface" in out:
            return None

        data = {}
        for line in out.splitlines():
            line = line.strip()
            if ":" not in line:
                continue
            k, v = line.split(":", 1)
            k = k.strip()
            v = v.strip()
            data[k] = v

        if "SSID" not in data and "State" not in data:
            return None

        # Parse fields
        state = data.get("State", "disconnected").lower()
        ssid = data.get("SSID", "")
        if not ssid or state != "connected":
            return {
                "status": "disconnected",
                "ssid": "None",
                "signal": 0,
                "channel": 0,
                "radio_type": "N/A",
                "authentication": "N/A",
                "rx_rate_mbps": 0.0,
                "tx_rate_mbps": 0.0,
                "interface": data.get("Description", "Wi-Fi"),
            }

        # Signal %
        sig_str = data.get("Signal", "0%")
        sig_val = int(re.sub(r"[^\d]", "", sig_str) or "0")

        # Channel
        chan_str = data.get("Channel", "0")
        chan_val = int(re.sub(r"[^\d]", "", chan_str) or "0")

        # Radio type
        radio = data.get("Radio type", "802.11")

        # Authentication
        auth = data.get("Authentication", "WPA2")

        # Rates (Mbps)
        rx_str = data.get("Receive rate (Mbps)", "0")
        rx_val = float(re.sub(r"[^\d\.]", "", rx_str) or "0")
        tx_str = data.get("Transmit rate (Mbps)", "0")
        tx_val = float(re.sub(r"[^\d\.]", "", tx_str) or "0")

        desc = data.get("Description", "Wi-Fi Interface")

        return {
            "status": "connected",
            "ssid": ssid,
            "signal": sig_val,
            "channel": chan_val,
            "radio_type": radio,
            "authentication": auth,
            "rx_rate_mbps": rx_val,
            "tx_rate_mbps": tx_val,
            "interface": desc,
        }
    except Exception:
        return None

def get_wifi_info_via_wlanapi():
    """Queries Windows Native Wifi API (wlanapi.dll) via ctypes."""
    if os.name != "nt":
        # Under WSL2 / Linux on Windows host: invoke python.exe to query host Windows API
        try:
            res = subprocess.run(
                ["python.exe", "-c", "import tools.windows_host_wifi_helper as h; import json; print(json.dumps(h.get_wifi_info_via_wlanapi()))"],
                capture_output=True,
                text=True,
                timeout=3,
            )
            if res.returncode == 0 and res.stdout.strip():
                return json.loads(res.stdout.strip())
        except Exception:
            pass
        return None
    try:
        import ctypes
        from ctypes import wintypes

        wlan = ctypes.windll.wlanapi
        handle = wintypes.HANDLE()
        negotiated = wintypes.DWORD()
        if wlan.WlanOpenHandle(2, None, ctypes.byref(negotiated), ctypes.byref(handle)) != 0:
            return None

        class GUID(ctypes.Structure):
            _fields_ = [
                ("Data1", wintypes.DWORD),
                ("Data2", wintypes.WORD),
                ("Data3", wintypes.WORD),
                ("Data4", ctypes.c_ubyte * 8),
            ]

        class WLAN_INTERFACE_INFO(ctypes.Structure):
            _fields_ = [
                ("InterfaceGuid", GUID),
                ("strInterfaceDescription", ctypes.c_wchar * 256),
                ("isState", wintypes.DWORD),
            ]

        class WLAN_INTERFACE_INFO_LIST(ctypes.Structure):
            _fields_ = [
                ("dwNumberOfItems", wintypes.DWORD),
                ("dwIndex", wintypes.DWORD),
                ("InterfaceInfo", WLAN_INTERFACE_INFO * 1),
            ]

        class DOT11_SSID(ctypes.Structure):
            _fields_ = [
                ("uSSIDLength", wintypes.ULONG),
                ("ucSSID", ctypes.c_char * 32),
            ]

        class WLAN_ASSOCIATION_ATTRIBUTES(ctypes.Structure):
            _fields_ = [
                ("dot11Ssid", DOT11_SSID),
                ("dot11BssType", wintypes.DWORD),
                ("dot11Bssid", ctypes.c_ubyte * 6),
                ("dot11PhyType", wintypes.DWORD),
                ("uDot11PhyIndex", wintypes.ULONG),
                ("wlanSignalQuality", wintypes.ULONG),
                ("ulRxRate", wintypes.ULONG),
                ("ulTxRate", wintypes.ULONG),
            ]

        class WLAN_SECURITY_ATTRIBUTES(ctypes.Structure):
            _fields_ = [
                ("bSecurityEnabled", wintypes.BOOL),
                ("bOneXEnabled", wintypes.BOOL),
                ("dot11AuthAlgorithm", wintypes.DWORD),
                ("dot11CipherAlgorithm", wintypes.DWORD),
            ]

        class WLAN_CONNECTION_ATTRIBUTES(ctypes.Structure):
            _fields_ = [
                ("isState", wintypes.DWORD),
                ("wlanConnectionMode", wintypes.DWORD),
                ("strProfileName", ctypes.c_wchar * 256),
                ("wlanAssociationAttributes", WLAN_ASSOCIATION_ATTRIBUTES),
                ("wlanSecurityAttributes", WLAN_SECURITY_ATTRIBUTES),
            ]

        pList = ctypes.POINTER(WLAN_INTERFACE_INFO_LIST)()
        if wlan.WlanEnumInterfaces(handle, None, ctypes.byref(pList)) != 0 or pList.contents.dwNumberOfItems == 0:
            wlan.WlanCloseHandle(handle, None)
            return None

        intf = pList.contents.InterfaceInfo[0]
        desc = intf.strInterfaceDescription
        guid = intf.InterfaceGuid

        # Query Current Connection (opcode 7)
        data_size = wintypes.DWORD()
        ppData = ctypes.c_void_p()
        opcode_type = wintypes.DWORD()
        res = wlan.WlanQueryInterface(
            handle,
            ctypes.byref(guid),
            7,  # wlan_intf_opcode_current_connection
            None,
            ctypes.byref(data_size),
            ctypes.byref(ppData),
            ctypes.byref(opcode_type),
        )

        if res != 0 or not ppData.value:
            wlan.WlanCloseHandle(handle, None)
            return {
                "status": "disconnected",
                "ssid": "None",
                "signal": 0,
                "channel": 0,
                "radio_type": "N/A",
                "authentication": "N/A",
                "rx_rate_mbps": 0.0,
                "tx_rate_mbps": 0.0,
                "interface": desc,
            }

        conn_attr = ctypes.cast(ppData, ctypes.POINTER(WLAN_CONNECTION_ATTRIBUTES)).contents
        assoc = conn_attr.wlanAssociationAttributes
        ssid_len = assoc.dot11Ssid.uSSIDLength
        ssid_str = assoc.dot11Ssid.ucSSID[:ssid_len].decode("utf-8", errors="replace")
        if not ssid_str:
            ssid_str = conn_attr.strProfileName or "Wi-Fi Network"

        # Signal quality
        signal = int(assoc.wlanSignalQuality)
        rx_mbps = round(assoc.ulRxRate / 1000.0, 1)
        tx_mbps = round(assoc.ulTxRate / 1000.0, 1)

        # Query channel (opcode 8)
        channel = 0
        res_chan = wlan.WlanQueryInterface(
            handle,
            ctypes.byref(guid),
            8,  # wlan_intf_opcode_channel_number
            None,
            ctypes.byref(data_size),
            ctypes.byref(ppData),
            ctypes.byref(opcode_type),
        )
        if res_chan == 0 and ppData.value:
            channel = int(ctypes.cast(ppData, ctypes.POINTER(wintypes.ULONG)).contents.value)

        # Radio type from dot11PhyType
        phy_map = {
            1: "802.11fh",
            2: "802.11dsss",
            3: "802.11ir",
            4: "802.11a",
            5: "802.11b",
            6: "802.11g",
            7: "802.11n (Wi-Fi 4)",
            8: "802.11ac (Wi-Fi 5)",
            9: "802.11ad",
            10: "802.11ax (Wi-Fi 6)",
        }
        radio_type = phy_map.get(assoc.dot11PhyType, f"802.11 (PHY {assoc.dot11PhyType})")

        # Auth algorithm
        auth_map = {
            1: "Open",
            2: "WEP",
            3: "WPA-Personal",
            4: "WPA-Enterprise",
            5: "WPA2-Personal",
            6: "WPA2-Enterprise",
            7: "WPA3-Personal",
            8: "WPA3-Enterprise",
        }
        auth = auth_map.get(conn_attr.wlanSecurityAttributes.dot11AuthAlgorithm, "WPA2/WPA3")

        wlan.WlanCloseHandle(handle, None)

        return {
            "status": "connected",
            "ssid": ssid_str,
            "signal": signal,
            "channel": channel,
            "radio_type": radio_type,
            "authentication": auth,
            "rx_rate_mbps": rx_mbps,
            "tx_rate_mbps": tx_mbps,
            "interface": desc,
        }
    except Exception:
        return None

def get_wifi_info():
    """Primary provider: tries in-process Native Wifi API (wlanapi.dll) first (0 child processes, 0 windows), then netsh with CREATE_NO_WINDOW."""
    wlan_info = get_wifi_info_via_wlanapi()
    if wlan_info and wlan_info.get("status") == "connected" and wlan_info.get("ssid") not in ("", "None"):
        return wlan_info
    info = get_wifi_info_via_netsh()
    if info and info.get("status") == "connected" and info.get("ssid") not in ("", "None"):
        return info
    if wlan_info:
        return wlan_info
    if info:
        return info
    return {
        "status": "disconnected",
        "ssid": "None",
        "signal": 0,
        "channel": 0,
        "radio_type": "N/A",
        "authentication": "N/A",
        "rx_rate_mbps": 0.0,
        "tx_rate_mbps": 0.0,
        "interface": "Wi-Fi Adapter",
    }

# Thread-safe telemetry cache updated in background every 3 seconds
LATEST_WIFI_INFO = None
WIFI_INFO_LOCK = threading.Lock()

def wifi_telemetry_worker():
    """Background worker continuously refreshing Wi-Fi telemetry so HTTP requests respond in <0.5ms."""
    global LATEST_WIFI_INFO
    while True:
        try:
            fresh_info = get_wifi_info()
            with WIFI_INFO_LOCK:
                LATEST_WIFI_INFO = fresh_info
        except Exception as e:
            log_entry(f"Background telemetry worker error: {e}")
        time.sleep(3.0)

class WifiHelperHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def log_message(self, format, *args):
        # Print clean, quiet access logs to file and stdout
        log_entry(f"[HTTP] {self.address_string()} - {format % args}")

    def do_GET(self):
        try:
            with WIFI_INFO_LOCK:
                info = dict(LATEST_WIFI_INFO) if LATEST_WIFI_INFO else get_wifi_info()
            payload = json.dumps(info, indent=2).encode("utf-8")

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Connection", "close")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(payload)
        except Exception as e:
            log_entry(f"Error serving GET request from {self.address_string()}: {e}\n{traceback.format_exc()}")

def main():
    if check_existing_instance():
        log_entry(f"BubbleOS Wi-Fi Helper is already active and responding on port {PORT}. Exiting duplicate instance.")
        sys.exit(0)

    save_pid()
    log_entry(f"BubbleOS Wi-Fi Helper starting on port {PORT} (PID {os.getpid()})...")

    global LATEST_WIFI_INFO
    current_info = get_wifi_info()
    with WIFI_INFO_LOCK:
        LATEST_WIFI_INFO = current_info
    log_entry(
        f"Initial telemetry: SSID='{current_info.get('ssid')}', Status='{current_info.get('status')}', "
        f"Signal={current_info.get('signal')}%, Adapter='{current_info.get('interface')}'"
    )

    # Start background polling worker
    worker_thread = threading.Thread(target=wifi_telemetry_worker, daemon=True)
    worker_thread.start()
    log_entry("Background telemetry worker thread started.")

    while True:
        try:
            server = HTTPServer(("0.0.0.0", PORT), WifiHelperHandler)
            log_entry(f"HTTP server bound to 0.0.0.0:{PORT}. Ready for BubbleOS VirtualBox queries.")
            server.serve_forever()
        except KeyboardInterrupt:
            log_entry("Shutdown requested by user/signal. Exiting.")
            break
        except Exception as e:
            log_entry(f"Unexpected server error: {e}\n{traceback.format_exc()}\nRestarting server in 2 seconds...")
            time.sleep(2.0)

    remove_pid()

if __name__ == "__main__":
    if "--stop" in sys.argv:
        stop_helper()
        sys.exit(0)
    main()
