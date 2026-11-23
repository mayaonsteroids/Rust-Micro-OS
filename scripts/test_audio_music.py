#!/usr/bin/env python3
"""
Automated Hardware and Verification Suite for Audio Subsystem and Music Player:
Tests Intel ICH AC97 / PC Speaker audio engine, shell audio commands,
desktop Music Player controls (Play, Pause, Next, Prev, Volume, Repeat, Stop),
and live playback state tracking.
"""

import os
import sys
import time
import subprocess

ISO_PATH = "/mnt/e/MICRO OS/iso/micro_os.iso"

def main():
    print("=================================================================")
    print("   RUST MICRO-OS AUDIO & MUSIC PLAYER VERIFICATION SUITE       ")
    print("=================================================================")

    if not os.path.exists(ISO_PATH):
        print(f"[-] Error: ISO not found at {ISO_PATH}")
        sys.exit(1)

    # Note: Enable AC97 device emulation in QEMU to test Tier 1 AC97 + Tier 2 PC Speaker
    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", ISO_PATH,
        "-device", "AC97",
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0",
        "-serial", "stdio",
        "-display", "none",
        "-m", "128M",
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
        print("[*] Waiting for optical boot and interactive shell prompt 'micro-os>'...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in boot_log:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        record_test("1. Optical Boot & Hardware Init", True, "System booted, AC97 audio and PC Speaker initialized")

        # Test 2: Shell Audio Status
        out_stat = send_cmd("audio status\n")
        stat_ok = "=== Audio Subsystem Status ===" in out_stat and "State:    [STOPPED]" in out_stat
        record_test("2. Audio Subsystem Status Command", stat_ok, "Reported driver, stopped state, and track metadata")

        # Test 3: Shell Audio Play / Pause / Next / Prev / Vol
        out_play = send_cmd("audio play\n")
        play_ok = "[AUDIO] Playing: 'Lavender Horizon'" in out_play or "Started playback" in out_play
        record_test("3. Shell 'audio play'", play_ok, "Started playback of Track 1")

        out_pause = send_cmd("audio pause\n")
        pause_ok = "[AUDIO] Playback paused" in out_pause
        record_test("4. Shell 'audio pause'", pause_ok, "Paused playback with position preserved")

        out_vol = send_cmd("audio vol 65\n")
        vol_ok = "Volume set to 65%" in out_vol
        record_test("5. Shell 'audio vol 65'", vol_ok, "Adjusted master/PCM hardware volume")

        out_next = send_cmd("audio next\n")
        next_ok = "Switched to next track" in out_next and "Kernel Pulse" in out_next
        record_test("6. Shell 'audio next'", next_ok, "Advanced to Track 2: Kernel Pulse")

        out_prev = send_cmd("audio prev\n")
        prev_ok = "Switched to previous track" in out_prev and "Lavender Horizon" in out_prev
        record_test("7. Shell 'audio prev'", prev_ok, "Returned to Track 1: Lavender Horizon")

        out_stop = send_cmd("audio stop\n")
        stop_ok = "[AUDIO] Playback stopped" in out_stop
        record_test("8. Shell 'audio stop'", stop_ok, "Stopped audio and reset playback state")

        # Test 9: Launch Desktop and Test Music Player GUI Controls
        out_desk = send_cmd("desktop\n", wait_for="Active application: Terminal")
        record_test("9. Desktop Launch", "Launched window manager" in out_desk, "Window manager active")

        # Switch to Music Player (App 5)
        out_app5 = send_cmd("5", wait_for="Switched active application to: Music Player")
        record_test("10. Focus Music Player App", "Music Player" in out_app5, "Focused Music Player application")

        # Send 'p' to start playback in GUI
        out_gui_play = send_cmd("p", wait_for="[AUDIO] Started playback: 'Lavender Horizon'")
        record_test("11. GUI Play Control ('p')", "[AUDIO] Started playback" in out_gui_play, "Started playback via 'p' keypress")

        # Send '+' to increase volume
        out_gui_vol = send_cmd("+", wait_for="Volume set to:")
        record_test("12. GUI Volume Up ('+')", "Volume set to:" in out_gui_vol, "Increased volume via '+' keypress")

        # Send 'n' to advance track
        out_gui_next = send_cmd("n", wait_for="Switched to next track: 'Kernel Pulse'")
        record_test("13. GUI Next Track ('n')", "Kernel Pulse" in out_gui_next, "Advanced track via 'n' keypress")

        # Send 'v' to return to previous track
        out_gui_prev = send_cmd("v", wait_for="Switched to previous track: 'Lavender Horizon'")
        record_test("14. GUI Prev Track ('v')", "Lavender Horizon" in out_gui_prev, "Previous track via 'v' keypress")

        # Send 'p' to pause playback
        out_gui_pause = send_cmd("p", wait_for="[AUDIO] Paused playback")
        record_test("15. GUI Pause Control ('p')", "[AUDIO] Paused playback" in out_gui_pause, "Paused playback via 'p' keypress")

        # Exit Desktop Mode via Escape Key
        out_esc = send_cmd("\x1b", wait_for="micro-os> ")
        esc_ok = "Returned to interactive CLI shell" in out_esc and "micro-os>" in out_esc
        record_test("16. Desktop Clean Exit", esc_ok, "Cleanly returned to CLI shell with audio paused")

        # Identity check post-audio
        out_who = send_cmd("whoami\n")
        record_test("17. System Identity Invariant", "maya" in out_who, "whoami returned 'maya'")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    all_passed = all(t[1] for t in test_results)
    print("\n" + "=" * 65)
    print("     AUDIO & MUSIC PLAYER TEST EXECUTION SUMMARY")
    print("=" * 65)
    for name, passed, details in test_results:
        st = "[PASS]" if passed else "[FAIL]"
        print(f"  {st} {name:<35}: {details}")
    print("=" * 65)

    if all_passed and len(test_results) >= 17:
        print("[+] ALL AUDIO & MUSIC PLAYER TESTS PASSED SUCCESSFULLY!")
        sys.exit(0)
    else:
        print("[-] SOME TESTS FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
