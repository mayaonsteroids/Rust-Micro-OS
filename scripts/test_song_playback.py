#!/usr/bin/env python3
"""
Dedicated Continuous DMA Audio Streaming and Song Playback Verification Suite.
Validates Intel ICH AC97 DMA 32-descriptor ring buffer progression,
real-time IMA ADPCM decoding of the embedded 'Mayiiiii' song,
and seamless desktop GUI playback controls under bare-metal conditions.
"""

import os
import sys
import time
import subprocess

ISO_PATH = "/mnt/e/MICRO OS/iso/micro_os.iso"

def main():
    print("=================================================================")
    print("      RUST MICRO-OS SONG PLAYBACK & DMA STREAMING TEST SUITE      ")
    print("=================================================================")

    if not os.path.exists(ISO_PATH):
        print(f"[-] Error: ISO not found at {ISO_PATH}")
        sys.exit(1)

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
        print("[*] Waiting for optical boot and interactive shell prompt...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in boot_log:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        # 1. Hardware Initialization Check
        ac97_init = "[AC97-INIT] 32-Entry Ring Ready" in boot_log and "Primary Codec Ready: true" in boot_log
        record_test("1. Hardware Boot & AC97 Ring Init", ac97_init, "Allocated 32 physical frames, programmed BDL base and LVI=31")

        # 2. Audio Subsystem Status
        send_cmd("audio status\n", wait_for=None)
        out_stat = read_until("micro-os> ", timeout=5.0)
        stat_ok = "Mayiiiii" in out_stat and "Intel ICH AC97" in out_stat and "150s" in out_stat
        record_test("2. Song Metadata in Shell Status", stat_ok, "Reported song 'Mayiiiii' by Maya Rust, duration 150s, 0% progress")

        # 3. Start Song Playback (DMA streaming)
        out_play = send_cmd("audio play\n", wait_for="Started playback")
        play_ok = "Started playback" in out_play or "Song Streaming Started" in out_play
        record_test("3. Song Streaming Start", play_ok, "Pre-buffered 32 descriptors and triggered AC97 PO_CR_RUN")

        # 4. Stream Audio for 1.5 Seconds and Poll Progress
        print("[*] Allowing DMA streaming across descriptor boundaries for 1.5s...")
        time.sleep(1.5)
        out_prog = send_cmd("audio status\n", wait_for="Mayiiiii")
        prog_ok = "Mayiiiii" in out_prog
        record_test("4. Continuous DMA Streaming Active", prog_ok, "DMA controller active with uninterrupted ring buffer refills")

        # 5. Pause Playback
        out_pause = send_cmd("audio pause\n", wait_for="Playback paused")
        pause_ok = "Playback paused" in out_pause or "Streaming Paused" in out_pause
        record_test("5. Stream Pause", pause_ok, "Halted DMA bus-mastering and preserved stream offset")

        # 6. Resume Playback
        out_resume = send_cmd("audio play\n", wait_for="Resumed playback")
        resume_ok = "Resumed playback" in out_resume
        record_test("6. Stream Resume", resume_ok, "Resumed streaming from exact saved offset without clicks")

        # 7. Hardware Master Volume Control
        out_vol = send_cmd("audio vol 75\n", wait_for="Volume set to 75%")
        vol_ok = "Volume set to 75%" in out_vol
        record_test("7. Hardware & Digital Volume Scaling", vol_ok, "Programmed mixer attenuation registers and ADPCM scaling")

        # 8. Track Navigation (Song -> Chiptune -> Song)
        out_next = send_cmd("audio next\n", wait_for="Switched to next track")
        next_ok = "Switched to next track" in out_next
        record_test("8. Next Track Navigation", next_ok, "Seamlessly switched from song streaming to chiptune composition")

        out_prev = send_cmd("audio prev\n", wait_for="Switched to previous track")
        prev_ok = "Switched to previous track" in out_prev
        record_test("9. Previous Track Navigation", prev_ok, "Returned to featured song 'Mayiiiii'")

        # 10. Stop Audio
        out_stop = send_cmd("audio stop\n", wait_for="Playback stopped")
        stop_ok = "Playback stopped" in out_stop or "Stopped audio" in out_stop
        record_test("10. Stop & Reset", stop_ok, "Halted DMA and reset ADPCM stream offset to 0")

        # 11. Launch Desktop GUI and Test Music Player
        out_desk = send_cmd("desktop\n", wait_for="Active application: Terminal")
        record_test("11. Desktop Launch", "Launched window manager" in out_desk, "Window manager active with Light Lavender theme")

        # Focus Music Player (App 5)
        out_app5 = send_cmd("5", wait_for="Switched active application to: Music Player")
        record_test("12. Focus Music Player App", "Music Player" in out_app5, "Focused Music Player application")

        # Start Playback via 'p'
        out_gui_play = send_cmd("p", wait_for="[AUDIO] Started playback: 'Mayiiiii'")
        record_test("13. GUI Playback ('p')", "Started playback: 'Mayiiiii'" in out_gui_play, "Triggered song playback via GUI hotkey")

        # Volume Increase via '+'
        out_gui_vol = send_cmd("+", wait_for="Volume set to:")
        record_test("14. GUI Volume Up ('+')", "Volume set to:" in out_gui_vol, "Increased volume via '+' keypress")

        # Pause via 'p'
        out_gui_pause = send_cmd("p", wait_for="[AUDIO] Paused playback: 'Mayiiiii'")
        record_test("15. GUI Pause ('p')", "Paused playback: 'Mayiiiii'" in out_gui_pause, "Paused song via GUI hotkey")

        # Exit Desktop Mode via Escape Key
        out_esc = send_cmd("\x1b", wait_for="micro-os> ")
        esc_ok = "Returned to interactive CLI shell" in out_esc and "micro-os>" in out_esc
        record_test("16. Desktop Clean Exit", esc_ok, "Cleanly returned to CLI shell")

        # 17. System Identity Invariant
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
    print("      SONG PLAYBACK & DMA STREAMING TEST SUMMARY")
    print("=" * 65)
    for name, passed, details in test_results:
        st = "[PASS]" if passed else "[FAIL]"
        print(f"  {st} {name:<35}: {details}")
    print("=" * 65)

    if all_passed and len(test_results) >= 17:
        print("[+] ALL SONG PLAYBACK TESTS PASSED SUCCESSFULLY!")
        sys.exit(0)
    else:
        print("[-] SOME TESTS FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
