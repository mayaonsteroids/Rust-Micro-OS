#!/usr/bin/env python3
"""
Automated Test Suite for Milestone 17.1: Identity Commands (whoami, hostname, uname).
Boots the newly compiled development ISO (iso/micro_os.iso) in QEMU,
tests all identity commands, runtime modifications, line editing, and regression execution.
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
    monitor_port = 45457

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO not found at {iso_path}")
        sys.exit(1)

    print(f"[*] Testing Development ISO: {iso_path} ({os.path.getsize(iso_path)} bytes)")

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", iso_path,
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
        initial = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in initial:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        print("[+] Acquired shell prompt successfully.")

        # Test 1: whoami
        out = send_cmd("whoami\n")
        passed = ("maya" in out) and ("Unknown" not in out)
        record_test("whoami Command", passed, "Returns username 'maya'")

        # Test 2: hostname (default)
        out = send_cmd("hostname\n")
        passed = ("micro-os" in out) and ("Unknown" not in out)
        record_test("hostname Default", passed, "Returns default 'micro-os'")

        # Test 3: hostname <new_name> runtime modification
        out = send_cmd("hostname rust-box\n")
        passed = ("Hostname updated to 'rust-box'" in out) and ("Runtime configuration only" in out)
        record_test("hostname Set Runtime", passed, "Updated runtime hostname to 'rust-box'")

        # Test 4: hostname query after change
        out = send_cmd("hostname\n")
        passed = ("rust-box" in out)
        record_test("hostname Query Updated", passed, "Returns updated hostname 'rust-box'")

        # Test 5: uname default
        out = send_cmd("uname\n")
        passed = (
            "Rust Micro-OS" in out and
            "0.1.0" in out and
            "x86_64" in out and
            "Long Mode" in out and
            "17" in out
        )
        record_test("uname Default", passed, "Outputs OS, version, arch, mode, milestone")

        # Test 6: uname -a (all info including updated hostname)
        out = send_cmd("uname -a\n")
        passed = (
            "Rust Micro-OS" in out and
            "rust-box" in out and
            "0.1.0" in out and
            "x86_64" in out and
            "Long Mode" in out and
            "Milestone 17" in out
        )
        record_test("uname -a", passed, "Outputs full system string with active hostname")

        # Test 7: uname flags (-s, -r, -m, -n)
        out_s = send_cmd("uname -s\n")
        out_r = send_cmd("uname -r\n")
        out_m = send_cmd("uname -m\n")
        out_n = send_cmd("uname -n\n")
        passed = (
            "Rust Micro-OS" in out_s and
            "0.1.0" in out_r and
            "x86_64" in out_m and
            "rust-box" in out_n
        )
        record_test("uname Flags (-s,-r,-m,-n)", passed, "Correct individual flags output")

        # Test 8: help catalog
        out = send_cmd("help\n")
        passed = (
            "whoami" in out and
            "hostname" in out and
            "uname" in out and
            "run <program>" in out
        )
        record_test("help Command Catalog", passed, "Lists identity commands and usage")

        # Test 9: Line editing (backspace & correction)
        # Type 'whoamx', backspace, 'i', newline -> should run 'whoami' -> 'maya'
        out = send_cmd("whoamx\x08i\n")
        passed = ("maya" in out)
        record_test("Line Editing / Backspace", passed, "Successfully corrected input to 'whoami'")

        # Test 10: Regression - list user programs
        out = send_cmd("list\n")
        passed = ("hello" in out and "counter" in out and "calc" in out)
        record_test("Regression: list", passed, "Programs hello, counter, calc listed")

        # Test 11: Regression - run hello in Ring 3
        out = send_cmd("run hello\n")
        passed = ("Hello from isolated Ring 3 user space!" in out and "exit code 0" in out)
        record_test("Regression: run hello", passed, "Ring 3 user program executes and exits with 0")

        # Test 12: Regression - run calc in Ring 3
        out = send_cmd("run calc\n")
        passed = ("427" in out and "exit code 0" in out)
        record_test("Regression: run calc", passed, "Ring 3 calc program executes and exits with 0")

        # Test 13: Capture VGA screenshot and verify Lavender theme palette
        rel_ppm = "screenshots/vga_milestone17_1.ppm"
        ppm_path = os.path.join(base_dir, rel_ppm)
        png_path = os.path.join(base_dir, "screenshots", "vga_milestone17_1.png")
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
            # Sample background pixels: Light lavender should be prominent (#E8DEFF / #E9DFFF)
            colors = img.getcolors(maxcolors=100000)
            colors.sort(key=lambda x: x[0], reverse=True)
            top_color = colors[0][1] # Top color should be light lavender
            hex_top = "#{:02X}{:02X}{:02X}".format(*top_color)
            print(f"  [+] Dominant background color: {hex_top} ({colors[0][0]} pixels)")
            # Lavender hex roughly #E8DEFF / #E9DFFF (R > 200, G > 200, B > 220)
            if top_color[0] > 200 and top_color[1] > 200 and top_color[2] > 220:
                vga_ok = True
        else:
            print(f"  [-] Screendump PPM file `{ppm_path}` was not created.")
        record_test("Visual Lavender Theme", vga_ok, "Confirmed Light Lavender background palette")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    print("\n" + "=" * 60)
    print("       MILESTONE 17.1 TEST EXECUTION SUMMARY")
    print("=" * 60)
    all_passed = True
    for name, passed, details in test_results:
        status_str = "PASS" if passed else "FAIL"
        if not passed:
            all_passed = False
        print(f"  [{status_str}] {name:30} : {details}")

    print("=" * 60)
    if all_passed and len(test_results) == 13:
        print(f"[+] ALL {len(test_results)} MILESTONE 17.1 TESTS PASSED SUCCESSFULLY!")
        return 0
    else:
        print("[-] SOME TESTS FAILED.")
        return 1

if __name__ == "__main__":
    sys.exit(main())
