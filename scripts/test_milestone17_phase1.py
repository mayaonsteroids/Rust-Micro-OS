#!/usr/bin/env python3
"""
Automated Hardware and Regression Verification Suite for Milestone 17 - Phase 1:
Desktop Window Manager, Left-Hand Application Dock, Terminal, SysMon, and About Apps.
"""

import os
import sys
import time
import socket
import subprocess

ISO_PATH = "/mnt/e/MICRO OS/iso/micro_os.iso"
MONITOR_PORT = 45461

def main():
    print("=================================================================")
    print("   RUST MICRO-OS MILESTONE 17 - PHASE 1 AUTOMATED VERIFICATION   ")
    print("=================================================================")

    if not os.path.exists(ISO_PATH):
        print(f"[-] Error: ISO not found at {ISO_PATH}")
        sys.exit(1)

    print(f"[*] Booting Development ISO: {ISO_PATH} ({os.path.getsize(ISO_PATH)} bytes)")

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", ISO_PATH,
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0",
        "-serial", "stdio",
        "-display", "none",
        "-m", "128M",
        "-monitor", f"tcp:127.0.0.1:{MONITOR_PORT},server,nowait",
    ]

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0,
    )

    full_output = ""

    def read_until(target: str, timeout: float = 25.0) -> str:
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

    def send_cmd(cmd_str: str, wait_for: str = "micro-os> ", timeout: float = 8.0) -> str:
        print(f"\n[SHELL-INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for:
            return read_until(wait_for, timeout=timeout)
        time.sleep(0.3)
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

        record_test("1. Optical Boot & Shell Ready", True, "Acquired interactive shell prompt 'micro-os>'")

        # Test 2: Baseline Identity check at CLI
        out_who = send_cmd("whoami\n")
        record_test("2. CLI Identity Invariant", "maya" in out_who, "Confirmed 'whoami' returns 'maya' at CLI prompt")

        # Test 3: Launch Desktop Environment
        out_desk = send_cmd("desktop\n", wait_for="Active application: Terminal")
        desk_launched = "Launched window manager" in out_desk and "Terminal" in out_desk
        record_test("3. Desktop Window Manager Launch", desk_launched, "Activated window manager with Terminal app")

        # Test 4: Execute command inside Terminal window
        out_term_who = send_cmd("whoami\n", wait_for="[DESKTOP TERMINAL] Executed: 'whoami'")
        term_who_ok = "[DESKTOP TERMINAL] Executed: 'whoami'" in out_term_who
        record_test("4. In-Window Terminal Execution", term_who_ok, "Command dispatched inside desktop Terminal window")

        # Test 5: Tab Key Navigation through Applications
        out_tab1 = send_cmd("\t", wait_for="Switched active application to: Network")
        record_test("5a. Dock Switch -> Network", "Network" in out_tab1, "Tab switched focus to Network Control Centre")

        out_tab2 = send_cmd("\t", wait_for="Switched active application to: Browser")
        record_test("5b. Dock Switch -> Browser", "Browser" in out_tab2, "Tab switched focus to Single-Tab Browser")

        out_tab3 = send_cmd("\t", wait_for="Switched active application to: Files")
        record_test("5c. Dock Switch -> Files", "Files" in out_tab3, "Tab switched focus to Files Manager")

        out_tab4 = send_cmd("\t", wait_for="Switched active application to: Music Player")
        record_test("5d. Dock Switch -> Music Player", "Music Player" in out_tab4, "Tab switched focus to Spotify-Inspired Music Player")

        out_tab5 = send_cmd("\t", wait_for="Switched active application to: System Monitor")
        record_test("5e. Dock Switch -> System Monitor", "System Monitor" in out_tab5, "Tab switched focus to System Monitor")

        out_tab6 = send_cmd("\t", wait_for="Switched active application to: Settings")
        record_test("5f. Dock Switch -> Settings", "Settings" in out_tab6, "Tab switched focus to Settings")

        out_tab7 = send_cmd("\t", wait_for="Switched active application to: About")
        record_test("5g. Dock Switch -> About", "About" in out_tab7, "Tab switched focus to About Dialog")

        # Test 6: Number Key Direct Switch (Quick-Switch)
        out_num6 = send_cmd("6", wait_for="Switched active application to: System Monitor")
        record_test("6a. Quick-Switch [6] -> SysMon", "System Monitor" in out_num6, "Direct number key switched to System Monitor")

        out_num1 = send_cmd("1", wait_for="Switched active application to: Terminal")
        record_test("6b. Quick-Switch [1] -> Terminal", "Terminal" in out_num1, "Direct number key switched back to Terminal")

        # Test 7: Exit Desktop Mode via Escape Key
        out_esc = send_cmd("\x1b", wait_for="micro-os> ")
        esc_ok = "Returned to interactive CLI shell" in out_esc and "micro-os>" in out_esc
        record_test("7. Desktop Exit via Escape", esc_ok, "Exited desktop and restored standard CLI screen")

        # Test 8: CLI Shell Integrity After Exit
        out_post_who = send_cmd("whoami\n")
        post_who_ok = "maya" in out_post_who
        record_test("8. Post-Exit CLI Invariant", post_who_ok, "CLI shell continues running with whoami='maya'")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    all_passed = all(t[1] for t in test_results)
    print("\n" + "=" * 65)
    print("       MILESTONE 17 PHASE 1 TEST EXECUTION SUMMARY")
    print("=" * 65)
    for name, passed, details in test_results:
        st = "[PASS]" if passed else "[FAIL]"
        print(f"  {st} {name:<32}: {details}")
    print("=" * 65)

    if all_passed and len(test_results) >= 8:
        print("[+] ALL PHASE 1 TESTS PASSED SUCCESSFULLY!")
        sys.exit(0)
    else:
        print("[-] SOME TESTS FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
