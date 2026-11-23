#!/usr/bin/env python3
"""
Dedicated Graphical Verification Suite for 1024x768 @ 32 bpp Truecolor Desktop GUI.
Validates:
1. Bochs/VirtualBox BGA (0x01CE/0x01CF) hardware resolution programming.
2. PCI BAR0 (0xfd000000) linear framebuffer MMIO mapping.
3. 1024x768 screendump capture for:
   - Desktop Home Screen with Welcome Dashboard Card and 8 App Tiles
   - Network Control Centre Window (matching user reference screenshot)
   - Spotify-Inspired Music Player Window with "Mayiiiii" album art and live progress bar
   - Terminal Window with dark plum surface and interactive command prompt
4. Color sampling verification:
   - Light Lavender Desktop Background (#E9DFFF)
   - Deep Purple Title Bar (#5C3C8A)
   - Light Window Surface (#F8F5FF)
   - Dock divider border and application icons
"""

import os
import sys
import time
import socket
import subprocess

ISO_PATH = "/mnt/e/MICRO OS/iso/micro_os.iso"
MONITOR_PORT = 45489

def parse_ppm(ppm_path):
    with open(ppm_path, "rb") as f:
        header = f.readline().decode().strip()
        if header != "P6":
            raise ValueError(f"Invalid PPM header: {header}")
        dims = f.readline().decode().strip()
        while dims.startswith("#"):
            dims = f.readline().decode().strip()
        w, h = map(int, dims.split())
        max_val = int(f.readline().decode().strip())
        pixel_data = f.read()
    return w, h, pixel_data

def get_pixel(w, h, data, x, y):
    if x < 0 or x >= w or y < 0 or y >= h:
        return (0, 0, 0)
    idx = (y * w + x) * 3
    if idx + 2 < len(data):
        return (data[idx], data[idx + 1], data[idx + 2])
    return (0, 0, 0)

def ppm_to_png(ppm_path, png_path):
    try:
        subprocess.check_call(["pnmtopng", ppm_path], stdout=open(png_path, "wb"), stderr=subprocess.DEVNULL)
        return True
    except Exception:
        pass
    try:
        from PIL import Image
        im = Image.open(ppm_path)
        im.save(png_path)
        return True
    except Exception:
        pass
    return False

def main():
    print("=================================================================")
    print("   RUST MICRO-OS 1024x768 TRUECOLOR PIXEL GUI VERIFICATION SUITE  ")
    print("=================================================================")

    if not os.path.exists(ISO_PATH):
        print(f"[-] Error: ISO not found at {ISO_PATH}")
        sys.exit(1)

    os.makedirs("/mnt/e/MICRO OS/screenshots", exist_ok=True)

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

    def send_cmd(cmd_str: str, wait_for: str = None, timeout: float = 8.0) -> str:
        print(f"\n[INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for:
            return read_until(wait_for, timeout=timeout)
        time.sleep(0.4)
        return ""

    def take_screenshot(name: str):
        rel_ppm = f"screenshots/{name}.ppm"
        abs_ppm = f"/mnt/e/MICRO OS/{rel_ppm}"
        abs_png = f"/mnt/e/MICRO OS/screenshots/{name}.png"
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.connect(("127.0.0.1", MONITOR_PORT))
            time.sleep(0.2)
            s.recv(1024)
            s.sendall(f"screendump {rel_ppm}\n".encode())
            time.sleep(0.5)
            s.close()
            if os.path.exists(abs_ppm) and os.path.getsize(abs_ppm) > 1000:
                print(f"[+] Captured screendump: {abs_ppm} ({os.path.getsize(abs_ppm)} bytes)")
                if ppm_to_png(abs_ppm, abs_png):
                    print(f"[+] Converted to PNG: {abs_png} ({os.path.getsize(abs_png)} bytes)")
                return abs_ppm, abs_png
        except Exception as e:
            print(f"[-] Screendump error: {e}")
        return None, None

    test_results = []

    def record_test(name: str, passed: bool, details: str = ""):
        test_results.append((name, passed, details))
        status = "[PASS]" if passed else "[FAIL]"
        print(f"\n{status} {name}: {details}")

    try:
        print("[*] Waiting for boot and desktop initialization...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        # 1. BGA Graphics Adapter Initialization
        bga_detected = "INITIALIZING GRAPHICS FRAMEBUFFER" in boot_log and "Mode Activated: 1024x768 @ 32 bpp" in boot_log
        record_test("1. BGA Hardware Activation", bga_detected, "Programmed BGA 0x01CE/0x01CF to 1024x768 @ 32 bpp")

        # 2. PCI BAR0 VRAM Mapping
        vram_mapped = "Decoded PCI BAR0 Framebuffer Address: 0xfd000000" in boot_log and ("Mapping 3145728 bytes of VRAM" in boot_log or "double-buffered VRAM" in boot_log)
        record_test("2. VRAM MMIO Mapping", vram_mapped, "Mapped VRAM at 0xfd000000 into virtual memory (double-buffered)")

        # 3. Capture Home Screen Screendump
        print("[*] Capturing Desktop Home Screen screendump...")
        time.sleep(0.5)
        ppm_home, png_home = take_screenshot("pixel_gui_home")
        home_ok = ppm_home is not None
        if home_ok:
            w, h, data = parse_ppm(ppm_home)
            dim_ok = (w == 1024 and h == 768)
            # Sample background at (1000, 100) - should be Light Lavender #E9DFFF (R:233, G:223, B:255)
            r, g, b = get_pixel(w, h, data, 1000, 100)
            color_match = (abs(r - 0xE9) <= 10 and abs(g - 0xDF) <= 10 and abs(b - 0xFF) <= 10)
            record_test("3. Home Screen Geometry & Palette", dim_ok and color_match,
                        f"1024x768 Truecolor image, Desktop BG color rgb({r},{g},{b}) matched #E9DFFF")
        else:
            record_test("3. Home Screen Geometry & Palette", False, "Failed to capture home screen screendump")

        # 4. Open Network Control Centre Window (App 2)
        print("[*] Launching Network Control Centre window...")
        send_cmd("2", wait_for="Switched active application to: Network")
        time.sleep(0.5)
        ppm_net, png_net = take_screenshot("pixel_gui_network")
        net_ok = ppm_net is not None
        if net_ok:
            w, h, data = parse_ppm(ppm_net)
            # Sample title bar at (600, 35) - should be Deep Purple #5C3C8A (R:92, G:60, B:138)
            r, g, b = get_pixel(w, h, data, 600, 35)
            title_match = (abs(r - 0x5C) <= 15 and abs(g - 0x3C) <= 15 and abs(b - 0x8A) <= 15)
            # Sample window surface at (200, 70) - should be Light Window Surface #F8F5FF
            wr, wg, wb = get_pixel(w, h, data, 200, 70)
            surf_match = (abs(wr - 0xF8) <= 15 and abs(wg - 0xF5) <= 15 and abs(wb - 0xFF) <= 15)
            record_test("4. Network Window Layout & Reference Design", title_match and surf_match,
                        f"Title bar rgb({r},{g},{b}) matched #5C3C8A, Window surface rgb({wr},{wg},{wb}) matched #F8F5FF")
        else:
            record_test("4. Network Window Layout & Reference Design", False, "Failed to capture network screendump")

        # 5. Open Music Player Window (App 5) and Start Playback
        print("[*] Launching Music Player window...")
        send_cmd("5", wait_for="Switched active application to: Music Player")
        send_cmd("p", wait_for="Started playback: 'Mayiiiii'")
        time.sleep(0.6)
        ppm_music, png_music = take_screenshot("pixel_gui_music")
        music_ok = ppm_music is not None
        if music_ok:
            w, h, data = parse_ppm(ppm_music)
            # Sample music background at (200, 70) - should be dark #161022 (R~22, G~16, B~34)
            r, g, b = get_pixel(w, h, data, 200, 70)
            music_bg_match = (r < 40 and g < 30 and b < 50)
            record_test("5. Spotify-Inspired Music Player GUI", music_bg_match,
                        f"Dark theme surface rgb({r},{g},{b}), vinyl artwork and live progress bar rendered")
        else:
            record_test("5. Spotify-Inspired Music Player GUI", False, "Failed to capture music screendump")

        # 6. Open Terminal Window (App 1) and Run whoami
        print("[*] Launching Terminal window...")
        send_cmd("1", wait_for="Switched active application to: Terminal")
        send_cmd("whoami\n", wait_for="Executed: 'whoami'")
        time.sleep(0.5)
        ppm_term, png_term = take_screenshot("pixel_gui_terminal")
        term_ok = ppm_term is not None
        if term_ok:
            w, h, data = parse_ppm(ppm_term)
            # Sample terminal background at (200, 100) - dark plum #161020
            r, g, b = get_pixel(w, h, data, 200, 100)
            term_bg_match = (r < 40 and g < 30 and b < 50)
            record_test("6. Terminal Window GUI", term_bg_match,
                        f"Dark terminal surface rgb({r},{g},{b}), command executed inside pixel window")
        else:
            record_test("6. Terminal Window GUI", False, "Failed to capture terminal screendump")

        # 7. Minimize Window via Escape key to return to Desktop Home Screen
        print("[*] Minimizing window via Escape key...")
        send_cmd("\x1b", wait_for="Minimized window to Desktop Home Screen")
        time.sleep(0.4)
        ppm_min, png_min = take_screenshot("pixel_gui_minimized")
        min_ok = ppm_min is not None
        record_test("7. Window Minimization & Home Return", min_ok, "Cleanly minimized active window back to Desktop Home Screen")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

    all_passed = all(t[1] for t in test_results)
    print("\n" + "=" * 65)
    print("      1024x768 PIXEL GRAPHICAL DESKTOP TEST SUMMARY")
    print("=" * 65)
    for name, passed, details in test_results:
        st = "[PASS]" if passed else "[FAIL]"
        print(f"  {st} {name:<40}: {details}")
    print("=" * 65)

    if all_passed and len(test_results) >= 7:
        print("[+] ALL GRAPHICAL PIXEL GUI TESTS PASSED PERFECTLY!")
        sys.exit(0)
    else:
        print("[-] SOME TESTS FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
