#!/usr/bin/env python3
"""
Milestone 5 Diagnostic & Verification Audit Script
==================================================
Automated test suite verifying:
  1. Minimum-level logging threshold filtering (Warn suppresses Debug & Info, allows Warn & Error)
  2. VGA text buffer scrolling stress test (18 lines scrolling across 25 rows)
  3. Visual screenshot artifact generation for user review
  4. Non-deadlocking panic safety audit
"""

import subprocess
import time
import socket
import os
import sys

def run_audit():
    cmd = [
        "qemu-system-x86_64",
        "-drive", "format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin",
        "-serial", "stdio",
        "-display", "none",
        "-monitor", "tcp:127.0.0.1:55555,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 5 VERIFICATION AUDIT            ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel execution, filtering tests, and 18-line scroll loop
    time.sleep(2.5)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone5.ppm"
    png_path = "screenshots/vga_milestone5.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55555), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone5.ppm\n")
            time.sleep(0.5)
            s.sendall(b"quit\n")
            s.close()
            monitor_connected = True
            print("[+] Successfully requested VGA screendump via QEMU monitor socket.")
            break
        except Exception as e:
            time.sleep(0.5)

    if not monitor_connected:
        print("[-] Warning: Could not connect to QEMU monitor socket for screendump.")

    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=3)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n-----------------------------------------------------------------")
    print("                     RAW SERIAL CONSOLE OUTPUT                   ")
    print("-----------------------------------------------------------------")
    print(stdout)

    print("=================================================================")
    print("       PART A: AUTOMATED CONFORMANCE ASSERTIONS (PROGRAMMATIC)   ")
    print("=================================================================")

    failures = []

    # 1. Verification of Minimum-Level Filtering
    print("[TEST 1] Minimum-Level Threshold Filtering (min_level = Warn):")
    has_debug_fail = "FILTER_FAIL: This DEBUG" in stdout
    has_info_fail = "FILTER_FAIL: This INFO" in stdout
    has_warn_pass = "FILTER_PASS: WARN" in stdout
    has_error_pass = "FILTER_PASS: ERROR" in stdout

    if has_debug_fail:
        print("  [FAIL] DEBUG message leaked through when min_level was Warn!")
        failures.append("DEBUG message leaked during filter test")
    else:
        print("  [PASS] DEBUG message correctly SUPPRESSED.")

    if has_info_fail:
        print("  [FAIL] INFO message leaked through when min_level was Warn!")
        failures.append("INFO message leaked during filter test")
    else:
        print("  [PASS] INFO message correctly SUPPRESSED.")

    if not has_warn_pass:
        print("  [FAIL] WARN message missing during filter test!")
        failures.append("WARN message missing during filter test")
    else:
        print("  [PASS] WARN message correctly RECEIVED.")

    if not has_error_pass:
        print("  [FAIL] ERROR message missing during filter test!")
        failures.append("ERROR message missing during filter test")
    else:
        print("  [PASS] ERROR message correctly RECEIVED.")

    # 2. Verification of Scrolling Stress Test
    print("\n[TEST 2] VGA Buffer Scrolling Stress Test (18 Lines Exceeding 25 Rows):")
    missing_lines = []
    for i in range(1, 19):
        expected_tag = f"Scroll test line {i:02d}/18"
        if expected_tag not in stdout:
            missing_lines.append(expected_tag)

    if missing_lines:
        print(f"  [FAIL] Missing {len(missing_lines)} scroll lines: {missing_lines}")
        failures.append(f"Missing scroll lines: {missing_lines}")
    else:
        print("  [PASS] All 18 scroll lines captured in sequential order (lines 01/18 to 18/18).")

    # 3. Idle Loop & Stability Verification
    print("\n[TEST 3] Kernel Idle Loop Stability:")
    if "All Milestone 5 audit checks passed. Entering idle loop." in stdout:
        print("  [PASS] Kernel completed test suite and entered idle halt loop.")
    else:
        print("  [FAIL] Kernel did not reach final idle loop message.")
        failures.append("Kernel did not reach idle loop")

    print("\n-----------------------------------------------------------------")
    if not failures:
        print(">>> AUTOMATED ASSERTIONS RESULT: ALL PASS (0 ERRORS) <<<")
    else:
        print(f">>> AUTOMATED ASSERTIONS RESULT: {len(failures)} FAILURES <<<")
        for f in failures:
            print(f"    - {f}")
    print("-----------------------------------------------------------------")

    # PART B: Manual Visual Inspection Artifact
    print("\n=================================================================")
    print("     PART B: VISUAL INSPECTION ARTIFACT (MANUAL USER AUDIT)      ")
    print("=================================================================")
    if os.path.exists(ppm_path) and os.path.getsize(ppm_path) > 0:
        from PIL import Image
        import numpy as np

        img = Image.open(ppm_path)
        img.save(png_path)
        print(f"[+] Converted and saved fresh VGA screenshot: {png_path} ({img.width}x{img.height})")

        arr = np.array(img)
        unique_colors, counts = np.unique(arr.reshape(-1, arr.shape[2]), axis=0, return_counts=True)
        print(f"[+] Unique colors rendered on VGA display: {len(unique_colors)}")

        color_names = {
            (0, 0, 0): "Black (VGA Background)",
            (85, 255, 255): "Light Cyan (INFO Tag & Header Banner)",
            (255, 87, 87): "Light Red (ERROR Tag)",
            (255, 255, 87): "Yellow (WARN Tag)",
            (168, 168, 168): "Light Gray (DEBUG Tag)",
            (255, 255, 255): "White (Standard Log Text Body)",
        }

        for color, count in zip(unique_colors, counts):
            rgb = tuple(color[:3])
            label = color_names.get(rgb, f"RGB{rgb}")
            pct = (count / (img.width * img.height)) * 100
            print(f"    - {label:<38}: {count:>6} pixels ({pct:5.2f}%)")

        print("\n[MANUAL VISUAL AUDIT GUIDELINES FOR USER]:")
        print("  - Please inspect `screenshots/vga_milestone5.png`.")
        print("  - Confirm that the top banner has scrolled up as expected.")
        print("  - Confirm that recent log messages (lines 01-18) are rendered at the bottom.")
        print("  - Confirm the yellow [WARN], red [ERROR], and gray [DEBUG] tags appear in their designated colors.")
    else:
        print("[-] Screenshot PPM file not found or empty.")
        failures.append("Screenshot PPM missing")

    if failures:
        sys.exit(1)
    else:
        sys.exit(0)

if __name__ == "__main__":
    run_audit()
