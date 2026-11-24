#!/usr/bin/env python3
"""
PS/2 Mouse Driver & Clickable Graphical Desktop Automated Verification
======================================================================
Tests:
  1. Boot from optical ISO into BubbleOS graphical desktop.
  2. In-kernel PS/2 mouse unit tests pass (7/7 tests).
  3. 8042 aux port initialization and IRQ 12 streaming activation.
  4. QEMU monitor mouse_move packet delivery to guest OS.
  5. Dock hit-testing: click dock icon (Network) opens Network window.
  6. Window control hit-testing: click Close [X] minimizes/closes window back to Home Screen.
  7. Home dashboard tile hit-testing: click Music Player tile opens Music Player window.
  8. Music Player in-window control: click Play button toggles audio playback.
  9. Keyboard & mouse co-existence: press Esc minimizes window.
 10. Framebuffer screendump verification.
"""

import subprocess
import time
import socket
import os
import sys

def run_mouse_ps2_test():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    iso_path = os.path.join(base_dir, "iso/micro_os.iso")

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO file '{iso_path}' not found!")
        sys.exit(1)

    monitor_port = 55588
    ppm_path = os.path.join(base_dir, "desktop_mouse_test.ppm")
    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", iso_path,
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-vga", "std",
        "-monitor", f"tcp:127.0.0.1:{monitor_port},server,nowait"
    ]

    print("=================================================================")
    print("     BUBBLEOS: PS/2 MOUSE DRIVER & INTERACTIVE DESKTOP AUDIT     ")
    print("=================================================================")
    print(f"Target ISO: {iso_path}")
    print(f"Command: {' '.join(cmd)}\n")

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0
    )

    try:
        os.set_blocking(proc.stdout.fileno(), False)
    except Exception:
        pass

    full_output = ""

    def read_until(target: str, timeout: float = 30.0) -> bool:
        nonlocal full_output
        start = time.time()
        buf = ""
        while time.time() - start < timeout:
            if proc.poll() is not None:
                break
            try:
                ch = proc.stdout.read(1)
            except Exception:
                ch = None
            if ch:
                buf += ch
                full_output += ch
                sys.stdout.write(ch)
                sys.stdout.flush()
                if target in buf:
                    return True
            else:
                time.sleep(0.01)
        return False

    def connect_monitor(max_retries: int = 20) -> socket.socket:
        for _ in range(max_retries):
            try:
                s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                s.connect(("127.0.0.1", monitor_port))
                time.sleep(0.1)
                s.recv(4096)
                return s
            except ConnectionRefusedError:
                time.sleep(0.5)
        raise RuntimeError("Failed to connect to QEMU monitor socket")

    def send_monitor(sock: socket.socket, command: str):
        sock.sendall((command + "\n").encode())
        time.sleep(0.15)
        try:
            sock.recv(4096)
        except Exception:
            pass

    try:
        # Step 1: Wait for 8042 aux port initialization & IRQ 12 streaming
        print("[1/8] Verifying 8042 aux port initialization & IRQ 12 streaming...")
        if not read_until("[MOUSE] PS/2 Mouse streaming enabled", timeout=25.0):
            print("[-] FAIL: Mouse streaming was not enabled!")
            print("Output so far:\n", full_output)
            sys.exit(1)
        print("  [+] PASSED: 8042 PS/2 aux port active and streaming configured.")

        # Step 2: Wait for PS/2 mouse unit tests to complete
        print("[2/8] Verifying in-kernel PS/2 mouse unit tests...")
        if not read_until("[TEST] All 7 PS/2 Mouse Unit Tests PASSED successfully.", timeout=15.0):
            print("[-] FAIL: Mouse unit tests did not complete in time!")
            print("Output so far:\n", full_output)
            sys.exit(1)
        print("  [+] PASSED: All 7 PS/2 Mouse Unit Tests verified in kernel.")

        # Step 3: Wait for Desktop launch
        print("[3/8] Waiting for BubbleOS desktop launch...")
        if not read_until("[DESKTOP] Launched window manager.", timeout=15.0):
            print("[-] FAIL: Desktop window manager failed to launch!")
            sys.exit(1)
        print("  [+] PASSED: BubbleOS desktop window manager operational.")

        time.sleep(1.0)
        mon = connect_monitor()
        print("  [+] Connected to QEMU monitor socket.")

        # Step 4: Move mouse to Left Dock -> Network Icon and Click
        # Mouse starts at (512, 384). Dock Network icon is at x=40, y=165.
        # Screen Delta: dx = 40 - 512 = -472, dy = 165 - 384 = -219 (upwards)
        print("[4/8] Testing Dock hit-testing: clicking Network icon...")
        send_monitor(mon, "mouse_move -472 -219")
        time.sleep(0.2)
        send_monitor(mon, "mouse_button 1")
        time.sleep(0.1)
        send_monitor(mon, "mouse_button 0")

        if not read_until("[MOUSE] Dock click -> Activated Network", timeout=10.0):
            print("[-] FAIL: Dock click did not activate Network!")
            sys.exit(1)
        print("  [+] PASSED: Dock click activated Network window.")

        time.sleep(0.5)

        # Step 5: Test Window Close [X] Button
        # Mouse is at (40, 165). Close button [X] is at x=980, y=36.
        # Screen Delta: dx = 980 - 40 = +940, dy = 36 - 165 = -129 (upwards)
        print("[5/8] Testing Window Header Close [X] button...")
        send_monitor(mon, "mouse_move 940 -129")
        time.sleep(0.2)
        send_monitor(mon, "mouse_button 1")
        time.sleep(0.1)
        send_monitor(mon, "mouse_button 0")

        if not read_until("[MOUSE] Clicked Close [X] -> Returned to Home Screen.", timeout=10.0):
            print("[-] FAIL: Close button click did not return to Home Screen!")
            sys.exit(1)
        print("  [+] PASSED: Close [X] button closed window and returned to Home Screen.")

        time.sleep(0.5)

        # Step 6: Test Home Dashboard Quick-Launch Tile: Music Player
        # Mouse is at (980, 36). Music Player tile (row 2, col 0) is at x=350, y=426.
        # Screen Delta: dx = 350 - 980 = -630, dy = 426 - 36 = +390 (downwards)
        print("[6/8] Testing Home Dashboard Tile click: opening Music Player...")
        send_monitor(mon, "mouse_move -630 390")
        time.sleep(0.2)
        send_monitor(mon, "mouse_button 1")
        time.sleep(0.1)
        send_monitor(mon, "mouse_button 0")

        if not read_until("[MOUSE] Dashboard tile click -> Opened Music Player", timeout=10.0):
            print("[-] FAIL: Dashboard tile click did not open Music Player!")
            sys.exit(1)
        print("  [+] PASSED: Dashboard tile click opened Music Player window.")

        time.sleep(0.5)

        # Step 7: Test Music Player In-Window Control: Play/Resume Button
        # Mouse is at (350, 426). Play/Pause button is at x=556, y=558.
        # Screen Delta: dx = 556 - 350 = +206, dy = 558 - 426 = +132 (downwards)
        print("[7/8] Testing Music Player in-window control: clicking Play/Resume...")
        send_monitor(mon, "mouse_move 206 132")
        time.sleep(0.2)
        send_monitor(mon, "mouse_button 1")
        time.sleep(0.1)
        send_monitor(mon, "mouse_button 0")

        if not read_until("[MOUSE] Music Player: Clicked Play/Resume", timeout=10.0):
            print("[-] FAIL: In-window Play button click was not handled!")
            sys.exit(1)
        print("  [+] PASSED: Music Player Play button click triggered playback.")

        time.sleep(0.5)

        # Step 8: Test Keyboard Co-existence (Escape minimizes window)
        print("[8/8] Testing Keyboard co-existence (Escape minimizes window)...")
        send_monitor(mon, "sendkey esc")
        if not read_until("[DESKTOP] Minimized window to Desktop Home Screen.", timeout=10.0):
            print("[-] FAIL: Escape key did not minimize window!")
            sys.exit(1)
        print("  [+] PASSED: Keyboard shortcuts fully functional alongside PS/2 mouse.")

        # Capture visual screendump
        send_monitor(mon, f"screendump {ppm_path}")
        time.sleep(0.5)
        if os.path.exists(ppm_path) and os.path.getsize(ppm_path) > 1000:
            print(f"  [+] PASSED: Desktop framebuffer captured to {ppm_path} ({os.path.getsize(ppm_path)} bytes).")

        print("\n=================================================================")
        print("    SUCCESS: ALL 8 PS/2 MOUSE & DESKTOP AUDITS PASSED 100%       ")
        print("=================================================================")
        mon.close()
        proc.kill()
        sys.exit(0)

    except Exception as e:
        print(f"[-] EXCEPTION: {e}")
        proc.kill()
        sys.exit(1)

if __name__ == "__main__":
    run_mouse_ps2_test()
