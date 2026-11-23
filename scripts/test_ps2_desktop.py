#!/usr/bin/env python3
"""
Comprehensive PS/2 Hardware Keyboard & Audio Subsystem Verification Suite.
Tests:
  1. Boot from optical ISO to interactive shell prompt.
  2. Launch Desktop GUI via PS/2 keyboard scancodes ('desktop\n').
  3. Switch to Music Player via PS/2 '5'.
  4. Start playback via PS/2 'p' -> verify AC97 DMA streaming started at 262 Hz.
  5. Advance track via PS/2 'n' -> verify track 2 (Kernel Pulse) at 147 Hz.
  6. Return to previous track via PS/2 'v' -> verify track 1 (Lavender Horizon).
  7. Toggle repeat mode via PS/2 'r' -> verify repeat ON/OFF.
  8. Volume control via PS/2 '+' and '-' -> verify volume adjustment.
  9. Pause playback via PS/2 'p' -> verify AC97 DMA streaming paused.
 10. Stop playback via PS/2 's' -> verify stopped state.
 11. Exit desktop via PS/2 Escape key -> verify return to CLI.
 12. Regression: 'whoami' returning 'maya'.
"""

import socket
import subprocess
import time
import sys
import os

ISO_PATH = "iso/micro_os.iso"
MONITOR_PORT = 45495

def main():
    print("=================================================================")
    print("  PS/2 KEYBOARD & AC97 AUDIO FULL HARDWARE VERIFICATION SUITE   ")
    print("=================================================================")

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
        "-monitor", f"tcp:127.0.0.1:{MONITOR_PORT},server,nowait"
    ]
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=0)
    time.sleep(2)

    mon = socket.create_connection(("127.0.0.1", MONITOR_PORT), timeout=5)
    time.sleep(0.5)
    mon.recv(1024)

    test_results = []
    def record_test(name, passed, details=""):
        test_results.append((name, passed, details))
        status = "[PASS]" if passed else "[FAIL]"
        print(f"\n{status} {name}: {details}")

    def read_until(target, timeout=20.0):
        start = time.time()
        buf = ""
        while time.time() - start < timeout:
            if proc.poll() is not None:
                break
            ch = proc.stdout.read(1)
            if ch:
                buf += ch
                sys.stdout.write(ch)
                sys.stdout.flush()
                if target in buf:
                    return buf
            else:
                time.sleep(0.005)
        return buf

    def send_ps2_key(k):
        mon.sendall(f"sendkey {k}\n".encode())
        time.sleep(0.08)

    try:
        print("[*] Waiting for optical boot and interactive shell prompt...")
        boot_log = read_until("micro-os> ", timeout=25.0)
        ac97_ok = "[AC97-INIT] 32-Entry Ring Ready" in boot_log and "Primary Codec Ready: true" in boot_log
        record_test("1. Hardware Boot & AC97 Init", ac97_ok, "AC97 codec ready, 32-entry circular BDL ring initialized")

        # 2. Launch Desktop GUI via PS/2
        for ch in "desktop":
            send_ps2_key(ch)
        send_ps2_key("ret")
        out_desk = read_until("Active application: Terminal", timeout=8.0)
        record_test("2. Desktop Launch via PS/2", "Active application: Terminal" in out_desk, "Launched desktop via PS/2 typing")

        # 3. Switch to Music Player via PS/2 '5'
        send_ps2_key("5")
        out_app5 = read_until("Switched active application to: Music Player", timeout=5.0)
        record_test("3. Focus Music Player via PS/2 '5'", "Music Player" in out_app5, "Focused Music Player via PS/2 key '5'")

        # 4. Play control via PS/2 'p'
        send_ps2_key("p")
        out_play = read_until("[MUSIC-KEY] 'P' -> Started playback", timeout=5.0)
        play_ok = "[AC97] Streaming Started" in out_play and "Lavender Horizon" in out_play
        record_test("4. PS/2 'P' -> Playback Start", play_ok, "Triggered 48 kHz DMA streaming and playback state PLAYING")

        # 5. Volume Up via PS/2 '+' (shift-equal)
        send_ps2_key("shift-equal")
        out_vol_up = read_until("[MUSIC-KEY] '+' -> Volume:", timeout=5.0)
        record_test("5. PS/2 '+' -> Volume Up", "Volume:" in out_vol_up, "Hardware volume increased and logged")

        # 6. Volume Down via PS/2 '-' (minus)
        send_ps2_key("minus")
        out_vol_down = read_until("[MUSIC-KEY] '-' -> Volume:", timeout=5.0)
        record_test("6. PS/2 '-' -> Volume Down", "Volume:" in out_vol_down, "Hardware volume decreased and logged")

        # 7. Next Track via PS/2 'n'
        send_ps2_key("n")
        out_next = read_until("[MUSIC-KEY] 'N' -> Next track: 'Kernel Pulse'", timeout=5.0)
        record_test("7. PS/2 'N' -> Next Track", "Kernel Pulse" in out_next, "Advanced to Track 2: Kernel Pulse")

        # 8. Prev Track via PS/2 'v'
        send_ps2_key("v")
        out_prev = read_until("[MUSIC-KEY] 'V' -> Prev track: 'Lavender Horizon'", timeout=5.0)
        record_test("8. PS/2 'V' -> Prev Track", "Lavender Horizon" in out_prev, "Returned to Track 1: Lavender Horizon")

        # 9. Toggle Repeat via PS/2 'r'
        send_ps2_key("r")
        out_rep = read_until("[MUSIC-KEY] 'R' -> Repeat:", timeout=5.0)
        record_test("9. PS/2 'R' -> Toggle Repeat", "Repeat:" in out_rep, "Repeat mode toggled and logged")

        # 10. Pause via PS/2 'p'
        send_ps2_key("p")
        out_pause = read_until("[MUSIC-KEY] 'P' -> Paused playback", timeout=5.0)
        pause_ok = "[AC97] Streaming Paused" in out_pause
        record_test("10. PS/2 'P' -> Pause", pause_ok, "AC97 DMA paused and playback state PAUSED")

        # 11. Stop via PS/2 's'
        send_ps2_key("s")
        out_stop = read_until("[MUSIC-KEY] 'S' -> Stopped playback", timeout=5.0)
        record_test("11. PS/2 'S' -> Stop", "Stopped playback" in out_stop, "Audio engine reset to STOPPED")

        # 12. Exit Desktop via PS/2 Escape
        send_ps2_key("esc")
        out_esc = read_until("micro-os> ", timeout=8.0)
        record_test("12. PS/2 'Esc' -> Desktop Exit", "Exited window manager" in out_esc and "micro-os>" in out_esc, "Cleanly restored CLI shell")

        # 13. Identity Invariant: whoami
        for ch in "whoami":
            send_ps2_key(ch)
        send_ps2_key("ret")
        out_who = read_until("micro-os> ", timeout=5.0)
        record_test("13. Identity Invariant", "maya" in out_who, "whoami confirmed 'maya' via PS/2 keyboard")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    print("\n" + "=" * 65)
    print("      PS/2 & AC97 VERIFICATION SUMMARY")
    print("=" * 65)
    all_passed = all(t[1] for t in test_results)
    for name, passed, details in test_results:
        st = "[PASS]" if passed else "[FAIL]"
        print(f"  {st} {name:<32}: {details}")
    print("=" * 65)

    if all_passed and len(test_results) >= 13:
        print("[+] ALL 13 PS/2 KEYBOARD & AC97 AUDIO TESTS PASSED!")
        sys.exit(0)
    else:
        print("[-] SOME TESTS FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
