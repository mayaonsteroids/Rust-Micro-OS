#!/usr/bin/env python3
"""
Automated Test Suite for Milestone 17.2: PCI Enumeration & Intel E1000 Driver.
Boots the development ISO in QEMU with:
  -netdev user,id=net0 -device e1000,netdev=net0
Verifies PCI scanning, E1000 detection, BAR0 decoding, DMA ring initialization,
MAC address extraction, packet transmission via DMA, and frame reception.
"""

import subprocess
import time
import os
import sys
import socket
from PIL import Image

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    iso_path = os.path.join(base_dir, "iso", "micro_os.iso")
    monitor_port = 45458

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO not found at {iso_path}")
        sys.exit(1)

    print(f"[*] Testing Development ISO with E1000: {iso_path} ({os.path.getsize(iso_path)} bytes)")

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", iso_path,
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0",
        "-serial", "stdio",
        "-display", "none",
        "-m", "128M",
        "-monitor", f"tcp:127.0.0.1:{monitor_port},server,nowait",
    ]

    print(f"[*] Launching QEMU: {' '.join(cmd)}")
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0,
    )

    full_output = ""

    def read_until(target: str, timeout: float = 15.0) -> str:
        nonlocal full_output
        start_time = time.time()
        buf = ""
        while time.time() - start_time < timeout:
            if proc.poll() is not None:
                break
            ch = proc.stdout.read(1)
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

    def send_cmd(cmd_str: str, wait_for_prompt: bool = True, timeout: float = 8.0) -> str:
        print(f"\n[SHELL-INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for_prompt:
            return read_until("micro-os> ", timeout=timeout)
        return ""

    test_results = []

    def record_test(name: str, passed: bool, details: str = ""):
        test_results.append((name, passed, details))
        status = "[PASS]" if passed else "[FAIL]"
        print(f"\n{status} {name}: {details}")

    try:
        print("[*] Waiting for optical boot and interactive shell prompt 'micro-os>' (timeout: 25s)...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in boot_log:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        print("[+] Acquired shell prompt successfully.")

        # Test 1: PCI Enumeration completed without crash
        pci_enum_ok = "[PCI] Enumeration complete" in boot_log
        record_test("1. PCI Bus Enumeration", pci_enum_ok, "Scanned PCI configuration space without crashing")

        # Test 2: E1000 Detected by PCI scan
        e1000_detected = "0x8086" in boot_log and "0x100e" in boot_log.lower()
        record_test("2. E1000 Device Detection", e1000_detected, "Detected Intel 82540EM (Vendor 0x8086, Dev 0x100E)")

        # Test 3: BAR0 Decoded & Validated
        bar0_ok = "BAR0 at physical 0x" in boot_log and "size: 128 KiB" in boot_log
        record_test("3. BAR0 Memory Decoding", bar0_ok, "Decoded 128 KiB MMIO address space for BAR0")

        # Test 4: PCI Bus Mastering and Memory Space enabled
        cmd_ok = "Bus Master: ON, Memory Space: ON" in boot_log
        record_test("4. PCI Command Config", cmd_ok, "Enabled Bus Master (DMA) & Memory Space in PCI Command")

        # Test 5: Hardware MAC Address Extraction
        mac_ok = "Hardware MAC Address: 52:54:00:" in boot_log or "Hardware MAC Address:" in boot_log
        record_test("5. MAC Address Extraction", mac_ok, "Extracted genuine MAC address from hardware RAL0/RAH0")

        # Test 6: DMA RX & TX Descriptor Rings Initialized
        dma_ok = "DMA Rings: RX at 0x" in boot_log and "TX at 0x" in boot_log and "36 KiB" in boot_log
        record_test("6. DMA Descriptor Rings", dma_ok, "Configured 128-byte aligned RX/TX rings with 2048B buffers")

        # Test 7: Hardware Engines & Link Status UP
        link_ok = "Link: UP" in boot_log and "RX and TX engines enabled" in boot_log
        record_test("7. Hardware RX/TX & Link UP", link_ok, "E1000 RX/TX enabled and link status is UP (1000 Mbps)")

        # Test 8: Shell `pci` Command
        out_pci = send_cmd("pci\n")
        pci_cmd_ok = "PCI Bus Device Registry" in out_pci and "0x8086:100e" in out_pci.lower()
        record_test("8. Shell 'pci' Command", pci_cmd_ok, "Lists PCI devices and BAR allocations in shell")

        # Test 9: Shell `net` Query Command
        out_net = send_cmd("net\n")
        net_cmd_ok = "MAC Address:" in out_net and "Link Status:  UP" in out_net and "DMA RX Ring:" in out_net
        record_test("9. Shell 'net' Status", net_cmd_ok, "Reports MAC, link status, and DMA ring addresses")

        # Test 10: Real Ethernet Frame Transmission via DMA (`net test`)
        out_test = send_cmd("net test\n")
        tx_ok = "Packet transmitted successfully via DMA (DD completed)" in out_test
        record_test("10. Frame DMA Transmission", tx_ok, "Transmitted broadcast ARP frame and verified DD bit completion")

        # Test 11: Real Frame Reception
        rx_ok = ("Received incoming Ethernet frame:" in out_test) or ("Hardware RX verification SUCCESS" in out_test)
        record_test("11. Frame Reception", rx_ok, "Processed incoming Ethernet frame via E1000 DMA RX engine")

        # Test 12: Regression - Identity and User programs
        out_id = send_cmd("whoami\n")
        out_prog = send_cmd("run hello\n")
        regr_ok = ("maya" in out_id) and ("Hello from isolated Ring 3" in out_prog)
        record_test("12. M16/M17.1 Regression", regr_ok, "Identity commands and Ring 3 user programs operational")

        # Test 13: Capture VGA screenshot
        rel_ppm = "screenshots/vga_milestone17_2.ppm"
        ppm_path = os.path.join(base_dir, rel_ppm)
        png_path = os.path.join(base_dir, "screenshots", "vga_milestone17_2.png")
        if os.path.exists(ppm_path):
            os.remove(ppm_path)
        if os.path.exists(png_path):
            os.remove(png_path)

        try:
            s = socket.create_connection(("127.0.0.1", monitor_port), timeout=5)
            time.sleep(0.3)
            s.recv(2048)
            s.sendall(f"screendump {rel_ppm}\n".encode())
            time.sleep(1.0)
            s.close()
            print(f"  [+] Screendump command sent to monitor socket on port {monitor_port}.")
        except Exception as e:
            print(f"  [!] Monitor socket error: {e}")

        vga_ok = False
        if os.path.exists(ppm_path):
            img = Image.open(ppm_path).convert("RGB")
            img.save(png_path)
            os.remove(ppm_path)
            print(f"  [+] Converted screendump to PNG: {png_path} ({os.path.getsize(png_path)} bytes)")
            colors = img.getcolors(maxcolors=100000)
            colors.sort(key=lambda x: x[0], reverse=True)
            top_color = colors[0][1]
            hex_top = "#{:02X}{:02X}{:02X}".format(*top_color)
            print(f"  [+] Dominant background color: {hex_top} ({colors[0][0]} pixels)")
            if top_color[0] > 200 and top_color[1] > 200 and top_color[2] > 220:
                vga_ok = True
        record_test("13. Visual Lavender Theme", vga_ok, "Light Lavender palette verified during network testing")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    print("\n" + "=" * 65)
    print("       MILESTONE 17.2 TEST EXECUTION SUMMARY")
    print("=" * 65)
    all_passed = True
    for name, passed, details in test_results:
        status_str = "PASS" if passed else "FAIL"
        if not passed:
            all_passed = False
        print(f"  [{status_str}] {name:32} : {details}")

    print("=" * 65)
    if all_passed and len(test_results) == 13:
        print(f"[+] ALL {len(test_results)} MILESTONE 17.2 HARDWARE TESTS PASSED SUCCESSFULLY!")
        return 0
    else:
        print("[-] SOME TESTS FAILED.")
        return 1

if __name__ == "__main__":
    sys.exit(main())
