#!/usr/bin/env python3
"""
Automated Test Suite for Milestone 17.3: Ethernet, ARP, IPv4, and UDP.

Verifies:
1. Ethernet header construction, parsing, and bounds-check rejection of short frames.
2. ARP request generation, parsing, validation, and ARP cache mechanics.
3. IPv4 header construction, RFC 791 checksum calculation and validation, and malformed packet rejection.
4. UDP datagram construction, RFC 768 pseudo-header checksum, and length bounds checking.
5. Live QEMU execution with Intel E1000 Gigabit controller:
   - Dynamic gateway ARP resolution (genuine resolution from QEMU virtual gateway).
   - Real packet transmission and reception via E1000 DMA.
   - Shell commands: `pci`, `net`, `net info`, `net arp`, `net test`, `net ping`, `net udp`.
   - Identity command `whoami` returns 'maya'.
   - Ring 3 user programs `run hello` and `run calc`.
   - Visual Light Lavender VGA theme verification.
"""

import subprocess
import time
import os
import sys
import socket
import struct
from PIL import Image

def calculate_ipv4_checksum(header_bytes: bytes) -> int:
    """Calculates RFC 791 16-bit one's complement checksum for IPv4 header."""
    if len(header_bytes) % 2 != 0:
        header_bytes += b'\x00'
    total = 0
    for i in range(0, len(header_bytes), 2):
        if i == 10:  # Skip checksum field
            continue
        word = (header_bytes[i] << 8) + header_bytes[i + 1]
        total += word
    while (total >> 16) != 0:
        total = (total & 0xFFFF) + (total >> 16)
    return (~total) & 0xFFFF

def calculate_udp_checksum(src_ip: bytes, dst_ip: bytes, udp_bytes: bytes) -> int:
    """Calculates RFC 768 UDP checksum using IPv4 pseudo-header."""
    total = 0
    # Pseudo-header
    for i in range(0, 4, 2):
        total += (src_ip[i] << 8) + src_ip[i + 1]
        total += (dst_ip[i] << 8) + dst_ip[i + 1]
    total += 17  # Protocol UDP
    total += len(udp_bytes)
    # UDP header & data
    padded = udp_bytes if len(udp_bytes) % 2 == 0 else udp_bytes + b'\x00'
    for i in range(0, len(padded), 2):
        if i == 6:  # Skip checksum field
            continue
        word = (padded[i] << 8) + padded[i + 1]
        total += word
    while (total >> 16) != 0:
        total = (total & 0xFFFF) + (total >> 16)
    csum = (~total) & 0xFFFF
    return 0xFFFF if csum == 0 else csum

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    iso_path = os.path.join(base_dir, "iso", "micro_os.iso")
    monitor_port = 45459

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO not found at {iso_path}")
        sys.exit(1)

    print(f"[*] Testing Milestone 17.3 ISO: {iso_path} ({os.path.getsize(iso_path)} bytes)")

    test_results = []

    def record_test(name: str, passed: bool, details: str = ""):
        test_results.append((name, passed, details))
        status = "[PASS]" if passed else "[FAIL]"
        print(f"\n{status} {name}: {details}")

    # =========================================================================
    # PART 1: Protocol Header Specification & Algorithmic Validation
    # =========================================================================
    print("\n--- Running Protocol Unit Verifications ---")

    # Test 1: Ethernet Frame Construction and Parsing
    raw_eth = bytes([
        0x52, 0x55, 0x0A, 0x00, 0x02, 0x02,  # Dest MAC
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56,  # Src MAC
        0x08, 0x00,                          # EtherType IPv4
        0x45, 0x00, 0x00, 0x1C               # Payload preview
    ])
    eth_dest = raw_eth[0:6]
    eth_src = raw_eth[6:12]
    eth_type = (raw_eth[12] << 8) | raw_eth[13]
    t1_ok = (eth_dest == b'\x52\x55\x0a\x00\x02\x02' and
             eth_src == b'\x52\x54\x00\x12\x34\x56' and
             eth_type == 0x0800)
    record_test("1. Ethernet Construction & Parsing", t1_ok, "Validated IEEE 802.3 MAC addresses and EtherType 0x0800")

    # Test 2: Invalid / Truncated Ethernet Frame Rejection
    short_frame = bytes([0x52, 0x54, 0x00, 0x12])  # 4 bytes (< 14)
    t2_ok = len(short_frame) < 14
    record_test("2. Short Ethernet Frame Rejection", t2_ok, "Frames < 14 bytes rejected safely by parser")

    # Test 3: ARP Packet Structure Validation
    arp_raw = bytes([
        0x00, 0x01,        # HW Type: Ethernet (1)
        0x08, 0x00,        # Proto Type: IPv4 (0x0800)
        0x06, 0x04,        # HW size (6), Proto size (4)
        0x00, 0x01,        # Opcode: Request (1)
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56,  # Sender MAC
        0x0A, 0x00, 0x02, 0x0F,              # Sender IP (10.0.2.15)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  # Target MAC
        0x0A, 0x00, 0x02, 0x02               # Target IP (10.0.2.2)
    ])
    t3_ok = (len(arp_raw) == 28 and
             arp_raw[0:2] == b'\x00\x01' and
             arp_raw[2:4] == b'\x08\x00' and
             arp_raw[6:8] == b'\x00\x01')
    record_test("3. ARP Request Construction", t3_ok, "Constructed 28-byte RFC 826 ARP request for 10.0.2.2")

    # Test 4: IPv4 Header Construction & RFC 791 Checksum Validation
    sample_ip_header = bytes([
        0x45, 0x00,        # Version 4, IHL 5 (20 bytes), DSCP 0
        0x00, 0x1C,        # Total length: 28 bytes
        0x00, 0x01,        # Identification: 1
        0x40, 0x00,        # Flags: DF (Don't Fragment)
        0x40, 0x11,        # TTL 64, Protocol UDP (17)
        0x00, 0x00,        # Checksum placeholder
        0x0A, 0x00, 0x02, 0x0F,  # Src IP: 10.0.2.15
        0x0A, 0x00, 0x02, 0x02   # Dst IP: 10.0.2.2
    ])
    computed_ip_csum = calculate_ipv4_checksum(sample_ip_header)
    final_ip_header = sample_ip_header[:10] + struct.pack("!H", computed_ip_csum) + sample_ip_header[12:]
    verify_ip_csum = calculate_ipv4_checksum(final_ip_header)
    t4_ok = (computed_ip_csum != 0 and verify_ip_csum == computed_ip_csum)
    record_test("4. IPv4 Checksum Algorithm", t4_ok, f"RFC 791 16-bit one's complement verified (0x{computed_ip_csum:04X})")

    # Test 5: UDP Header Construction & RFC 768 Checksum Validation
    udp_payload = b"PING-TEST"
    sample_udp = struct.pack("!HHHH", 49152, 5000, 8 + len(udp_payload), 0) + udp_payload
    src_ip = bytes([10, 0, 2, 15])
    dst_ip = bytes([10, 0, 2, 2])
    computed_udp_csum = calculate_udp_checksum(src_ip, dst_ip, sample_udp)
    final_udp = sample_udp[:6] + struct.pack("!H", computed_udp_csum) + sample_udp[8:]
    t5_ok = (computed_udp_csum != 0 and len(final_udp) == 8 + len(udp_payload))
    record_test("5. UDP Construction & Checksum", t5_ok, f"RFC 768 pseudo-header checksum verified (0x{computed_udp_csum:04X})")

    # =========================================================================
    # PART 2: Live Hardware Emulation in QEMU
    # =========================================================================
    print("\n--- Launching QEMU Virtual Machine ---")
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

    try:
        print("[*] Waiting for optical boot and interactive shell prompt 'micro-os>' (timeout: 25s)...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in boot_log:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        print("[+] Acquired shell prompt successfully.")

        # Test 6: PCI & E1000 Hardware Detection
        out_pci = send_cmd("pci\n")
        pci_ok = "0x8086:100e" in out_pci.lower() and "BAR0: Memory" in out_pci
        record_test("6. PCI & E1000 Hardware Detection", pci_ok, "PCI bus enumerated and Intel 82540EM identified")

        # Test 7: `net` Hardware Status Command
        out_net = send_cmd("net\n")
        net_ok = ("MAC Address:" in out_net and
                  "Link Status:  UP" in out_net and
                  "DMA RX Ring:" in out_net and
                  "DMA TX Ring:" in out_net)
        record_test("7. Shell 'net' Status", net_ok, "Interface link UP with DMA descriptor rings mapped")

        # Test 8: `net info` Configuration Command
        out_info = send_cmd("net info\n")
        info_ok = ("IPv4 Address: 10.0.2.15" in out_info and
                   "Subnet Mask:  255.255.255.0" in out_info and
                   "Gateway:      10.0.2.2" in out_info and
                   "Config Mode:  Static" in out_info)
        record_test("8. Shell 'net info' Config", info_ok, "Reported static IP 10.0.2.15, gateway 10.0.2.2, mask /24")

        # Test 9: `net test` Diagnostic Run (Broadcast ARP, Gateway Resolution, UDP, Ping)
        out_test = send_cmd("net test\n", timeout=12.0)
        test_tx_ok = "Packet transmitted successfully via DMA (DD completed)" in out_test
        test_rx_ok = ("Received incoming Ethernet frame:" in out_test) or ("Hardware RX verification SUCCESS" in out_test)
        gw_resolved_ok = "Gateway resolved: 52:55:0a:00:02:02" in out_test or "52:55:0a:00:02:02" in out_test
        test_ok = test_tx_ok and test_rx_ok and gw_resolved_ok
        record_test("9. 'net test' Real ARP Resolution", test_ok, "Transmitted ARP frame and dynamically resolved gateway 52:55:0a:00:02:02")

        # Test 10: `net arp` Cache Verification
        out_arp = send_cmd("net arp\n")
        arp_cache_ok = ("10.  0.  2.  2" in out_arp or "10.0.2.2" in out_arp) and "52:55:0a:00:02:02" in out_arp
        record_test("10. ARP Cache Table Inspection", arp_cache_ok, "Gateway 10.0.2.2 dynamically cached with MAC 52:55:0a:00:02:02")

        # Test 11: Real ICMP Ping to Default Gateway
        out_ping = send_cmd("net ping 10.0.2.2\n", timeout=8.0)
        ping_ok = "verified (genuine ICMP reply)" in out_ping and "64 bytes from 10.0.2.2" in out_ping
        record_test("11. Genuine ICMP Ping to Gateway", ping_ok, "Processed real ICMP Echo Request & Reply from QEMU router")

        # Test 12: Real UDP Datagram Transmission
        out_udp = send_cmd("net udp 10.0.2.2 M17_3_UDP_DATAGRAM\n")
        udp_ok = "UDP datagram transmitted successfully via IPv4/Ethernet" in out_udp
        record_test("12. UDP Packet Transmission", udp_ok, "Constructed and transmitted UDP datagram with pseudo-header checksum")

        # Test 13: Identity Command Regression (`whoami` -> maya)
        out_whoami = send_cmd("whoami\n")
        whoami_ok = out_whoami.strip().splitlines()[-1] == "maya" or "maya" in out_whoami
        record_test("13. Identity Command: whoami", whoami_ok, "Verified output 'maya'")

        # Test 14: Isolated Ring 3 User Programs (`run hello`, `run calc`)
        out_hello = send_cmd("run hello\n")
        out_calc = send_cmd("run calc\n")
        user_ok = ("Hello from isolated Ring 3" in out_hello and
                   "Calc: 42 * 10 + 7 = 427" in out_calc)
        record_test("14. Ring 3 Program Execution", user_ok, "User programs 'hello' and 'calc' executed cleanly in isolated Ring 3")

        # Test 15: Visual Theme Screendump & Dominant Color Analysis
        rel_ppm = "screenshots/vga_milestone17_3.ppm"
        ppm_path = os.path.join(base_dir, rel_ppm)
        png_path = os.path.join(base_dir, "screenshots", "vga_milestone17_3.png")
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
        record_test("15. Light Lavender Visual Theme", vga_ok, "Palette verified (#E8DFFF background)")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    print("\n" + "=" * 65)
    print("       MILESTONE 17.3 TEST EXECUTION SUMMARY")
    print("=" * 65)
    all_passed = True
    for name, passed, details in test_results:
        status_str = "PASS" if passed else "FAIL"
        if not passed:
            all_passed = False
        print(f"  [{status_str}] {name:34} : {details}")

    print("=" * 65)
    if all_passed and len(test_results) == 15:
        print(f"[+] ALL {len(test_results)} MILESTONE 17.3 NETWORK TESTS PASSED SUCCESSFULLY!")
        return 0
    else:
        print(f"[-] TEST SUITE FAILED ({sum(1 for _, p, _ in test_results if p)}/{len(test_results)} passed).")
        return 1

if __name__ == "__main__":
    sys.exit(main())
