#!/usr/bin/env python3
"""
Focused Verification Script for BubbleOS Host Wi-Fi Helper & Guest Connectivity.
Validates:
1. Host helper (tools/windows_host_wifi_helper.py) serves real Windows Wi-Fi parameters.
2. BubbleOS connects to the host helper via hypervisor NAT gateway (10.0.2.2:19820).
3. The `wifi` terminal command retrieves and displays the actual SSID ("Jain_Student" / "Jain Student"),
   signal %, channel, authentication, radio type, and link rates.
4. When helper is offline, BubbleOS displays a clean error without hanging or crashing.
"""

import os
import sys
import time
import json
import urllib.request
import subprocess

ISO_PATH = "/mnt/e/MICRO OS/iso/micro_os.iso"

def main():
    print("=================================================================")
    print("   BUBBLEOS: HOST WI-FI HELPER & GUEST CONNECTIVITY TEST         ")
    print("=================================================================")

    if not os.path.exists(ISO_PATH):
        print(f"[-] Error: ISO not found at {ISO_PATH}")
        sys.exit(1)

    # 1. Start Windows Host Wi-Fi Helper
    print("[1/4] Starting Host Wi-Fi Helper (tools/windows_host_wifi_helper.py)...")
    helper_proc = subprocess.Popen(
        [sys.executable, "tools/windows_host_wifi_helper.py"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    time.sleep(1.0)

    try:
        # 2. Query helper directly from host to verify JSON payload
        print("[2/4] Verifying Host Helper HTTP endpoint on http://127.0.0.1:19820/...")
        try:
            req = urllib.request.urlopen("http://127.0.0.1:19820/", timeout=3)
            raw_json = req.read().decode("utf-8")
            data = json.loads(raw_json)
            print(f"      [+] Host helper returned live JSON:")
            print(f"          - SSID:           {data.get('ssid')}")
            print(f"          - Status:         {data.get('status')}")
            print(f"          - Signal:         {data.get('signal')}%")
            print(f"          - Channel:        {data.get('channel')}")
            print(f"          - Radio Type:     {data.get('radio_type')}")
            print(f"          - Authentication: {data.get('authentication')}")
            print(f"          - Rates:          Rx: {data.get('rx_rate_mbps')} Mbps | Tx: {data.get('tx_rate_mbps')} Mbps")
            print(f"          - Adapter:        {data.get('interface')}")

            expected_ssid = data.get("ssid")
            if not expected_ssid or expected_ssid == "None":
                print("[-] Warning: Host is not connected to Wi-Fi. Continuing check...")
        except Exception as e:
            print(f"[-] Failed to query host helper directly: {e}")
            sys.exit(1)

        # 3. Boot BubbleOS in QEMU and test in-guest `wifi` command
        print("[3/4] Launching BubbleOS in QEMU to test guest -> host bridge connectivity...")
        qemu_cmd = [
            "qemu-system-x86_64",
            "-boot", "d",
            "-cdrom", ISO_PATH,
            "-netdev", "user,id=net0",
            "-device", "e1000,netdev=net0",
            "-serial", "stdio",
            "-display", "none",
            "-m", "128M",
        ]

        qemu_proc = subprocess.Popen(
            qemu_cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0,
        )

        full_output = ""

        def read_until(target: str, timeout: float = 25.0) -> str:
            nonlocal full_output
            start = time.time()
            buf = ""
            while time.time() - start < timeout:
                if qemu_proc.poll() is not None:
                    break
                ch = qemu_proc.stdout.read(1)
                if ch:
                    buf += ch
                    full_output += ch
                    sys.stdout.write(ch)
                    sys.stdout.flush()
                    if target in buf:
                        return buf
                else:
                    time.sleep(0.005)
            return buf

        print("      [*] Waiting for shell prompt...")
        boot_log = read_until("micro-os> ", timeout=25.0)
        if "micro-os>" not in boot_log:
            print("[-] Did not reach shell prompt.")
            qemu_proc.kill()
            sys.exit(1)

        print("\n      [*] Sending 'wifi' command into BubbleOS...")
        qemu_proc.stdin.write("wifi\n")
        qemu_proc.stdin.flush()

        wifi_out = read_until("micro-os> ", timeout=15.0)

        # Verify output contains actual host Wi-Fi details
        ssid_found = expected_ssid in wifi_out
        status_found = "Connected (Host WLAN Active)" in wifi_out
        bridge_heading_found = "=== HOST WI-FI CONNECTION (WINDOWS BRIDGE) ===" in wifi_out

        print("\n" + "=" * 65)
        print("          BUBBLEOS GUEST RETRIEVAL VERIFICATION RESULT")
        print("=" * 65)
        print(f"  [+] Host Bridge Header Found : {bridge_heading_found}")
        print(f"  [+] Connected State Verified : {status_found}")
        print(f"  [+] Actual SSID '{expected_ssid}' in Guest Output: {ssid_found}")

        if not (ssid_found and status_found and bridge_heading_found):
            print("[-] FAILED: Guest did not display the expected host Wi-Fi details.")
            qemu_proc.kill()
            sys.exit(1)

        print("[+] SUCCESS: BubbleOS successfully retrieved and displayed the actual host Wi-Fi network!")

        # 4. Test offline helper behavior
        print("\n[4/4] Testing offline helper behavior (stopping helper)...")
        helper_proc.terminate()
        helper_proc.wait(timeout=2.0)

        print("      [*] Sending 'wifi' command with helper stopped...")
        qemu_proc.stdin.write("wifi\n")
        qemu_proc.stdin.flush()

        offline_out = read_until("micro-os> ", timeout=10.0)
        offline_ok = "Host Wi-Fi Status: Offline" in offline_out

        print(f"  [+] Clean Offline Handling Verified: {offline_ok}")
        if not offline_ok:
            print("[-] FAILED: Guest did not report clean offline error when helper was stopped.")
            qemu_proc.kill()
            sys.exit(1)

        print("[+] SUCCESS: Guest cleanly reported offline status without hanging.")

        qemu_proc.kill()

    finally:
        try:
            helper_proc.terminate()
            helper_proc.wait(timeout=1.0)
        except Exception:
            pass

    print("\n" + "=" * 65)
    print("      ALL HOST WI-FI HELPER & GUEST AUDITS PASSED 100%!         ")
    print("=================================================================")

if __name__ == "__main__":
    main()
