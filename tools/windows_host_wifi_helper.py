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
import json
import re
import subprocess
from http.server import HTTPServer, BaseHTTPRequestHandler

PORT = 19820

def get_wifi_info_via_netsh():
    """Attempts to retrieve Wi-Fi connection info via netsh wlan show interfaces."""
    try:
        proc = subprocess.run(
            ["netsh", "wlan", "show", "interfaces"],
            capture_output=True,
            text=True,
            timeout=3,
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
    """Primary provider: tries netsh first, then native wlanapi."""
    info = get_wifi_info_via_netsh()
    if info and info.get("status") == "connected" and info.get("ssid") not in ("", "None"):
        return info
    wlan_info = get_wifi_info_via_wlanapi()
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

class WifiHelperHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def log_message(self, format, *args):
        # Print clean, quiet access logs
        sys.stdout.write(f"[HELPER] {self.address_string()} - {format % args}\n")
        sys.stdout.flush()

    def do_GET(self):
        info = get_wifi_info()
        payload = json.dumps(info, indent=2).encode("utf-8")

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Connection", "close")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(payload)

def main():
    print("=================================================================")
    print("   BUBBLEOS WINDOWS HOST WI-FI HELPER SERVICE (READ-ONLY)        ")
    print("=================================================================")
    
    current_info = get_wifi_info()
    print(f"[*] Detected Host Wi-Fi:")
    print(f"    - SSID:           {current_info.get('ssid')}")
    print(f"    - Status:         {current_info.get('status')}")
    print(f"    - Signal:         {current_info.get('signal')}%")
    print(f"    - Channel:        {current_info.get('channel')}")
    print(f"    - Radio Type:     {current_info.get('radio_type')}")
    print(f"    - Authentication: {current_info.get('authentication')}")
    print(f"    - Link Rates:     Rx: {current_info.get('rx_rate_mbps')} Mbps | Tx: {current_info.get('tx_rate_mbps')} Mbps")
    print(f"    - Adapter:        {current_info.get('interface')}")
    print(f"[*] Security Invariant: Passwords are NEVER queried or served.")
    print(f"[*] Listening on http://0.0.0.0:{PORT}/ ...")
    print(f"[*] Press Ctrl+C to stop.")
    print("=================================================================")

    server = HTTPServer(("0.0.0.0", PORT), WifiHelperHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[*] Stopping Wi-Fi Helper server...")
        server.server_close()

if __name__ == "__main__":
    main()
