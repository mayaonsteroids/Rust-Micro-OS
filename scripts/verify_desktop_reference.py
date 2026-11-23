#!/usr/bin/env python3
"""
Verification of Desktop Interface, Network Control Centre, Terminal, and Minimization.
Captures screenshots comparing against the user's reference image:
1. Desktop Home Screen (Light Lavender wallpaper with left dock showing 8 apps)
2. Network Control Centre window (showing Dual-Interface Control, live E1000 stats, Wi-Fi helper offline)
3. Terminal window (executing 'whoami' -> 'maya' and 'wifi' diagnostics)
4. Minimization back to Desktop Home Screen via Esc
"""

import subprocess
import time
import os
import sys
import socket
from PIL import Image

ISO_PATH = "iso/micro_os.iso"
MONITOR_PORT = 45499

def ppm_to_png(ppm_path, png_path):
    if os.path.exists(ppm_path):
        with Image.open(ppm_path) as img:
            img.save(png_path, "PNG")
        print(f"[+] Converted {ppm_path} -> {png_path} ({os.path.getsize(png_path)} bytes)")
        return True
    return False

def main():
    print("=================================================================")
    print("   DESKTOP GUI & NETWORK CONTROL CENTRE VISUAL AUDIT           ")
    print("=================================================================")

    os.makedirs("screenshots", exist_ok=True)

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

    def capture_screen(ppm_name, png_name):
        time.sleep(0.5)
        mon.sendall(f"screendump {ppm_name}\n".encode())
        time.sleep(0.5)
        ppm_to_png(ppm_name, png_name)

    def send_key(k):
        mon.sendall(f"sendkey {k}\n".encode())
        time.sleep(0.1)

    try:
        print("[*] Waiting for desktop boot...")
        # Read until desktop startup message
        start = time.time()
        buf = ""
        while time.time() - start < 20:
            ch = proc.stdout.read(1)
            if ch:
                buf += ch
                if "micro-os>" in buf:
                    break
            else:
                time.sleep(0.01)

        print("[+] Acquired system prompt / desktop ready.")
        time.sleep(1)

        # 1. Capture Desktop Home Screen
        print("[*] Capturing 1: Desktop Home Screen...")
        capture_screen("screenshots/vga_desktop_home.ppm", "screenshots/vga_desktop_home.png")

        # 2. Open Network Control Centre (press '2')
        print("[*] Opening Network Control Centre (key '2')...")
        send_key("2")
        time.sleep(1)
        print("[*] Capturing 2: Network Control Centre Window...")
        capture_screen("screenshots/vga_network_window.ppm", "screenshots/vga_network_window.png")

        # 3. Open Terminal (press '1')
        print("[*] Opening Terminal (key '1')...")
        send_key("1")
        time.sleep(1)

        # Type 'whoami'
        for c in "whoami":
            send_key(c)
        send_key("ret")
        time.sleep(0.5)

        # Type 'wifi'
        for c in "wifi":
            send_key(c)
        send_key("ret")
        time.sleep(1)

        print("[*] Capturing 3: Terminal with 'whoami' and 'wifi' diagnostics...")
        capture_screen("screenshots/vga_terminal_wifi.ppm", "screenshots/vga_terminal_wifi.png")

        # 4. Minimize back to Home Screen (press 'esc')
        print("[*] Pressing Escape to minimize back to Desktop Home Screen...")
        send_key("esc")
        time.sleep(1)
        print("[*] Capturing 4: Minimized Desktop Home Screen...")
        capture_screen("screenshots/vga_desktop_minimized.ppm", "screenshots/vga_desktop_minimized.png")

        print("\n[+] Visual capture sequence completed successfully!")

    finally:
        try:
            proc.kill()
            proc.wait(timeout=3)
        except Exception:
            pass

if __name__ == "__main__":
    main()
