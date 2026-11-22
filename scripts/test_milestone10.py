#!/usr/bin/env python3
"""
Milestone 10 Kernel Synchronization & Mutual Exclusion Verification Audit Script
================================================================================
Automated verification audit script covering:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task management tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Preserved Milestone 8 Cooperative Scheduler tests (ALL 12 TESTS PASSED) and live demo.
  5. Preserved Milestone 9 Interrupt unit tests (ALL 6 TESTS PASSED) and live preemption demo.
  6. Programmatic execution and verification of 7 in-kernel Milestone 10 Synchronization tests:
     - SYNC-TC1: Basic mutual exclusion (2 tasks, val=100) verified
     - SYNC-TC2: Multi-task contention (4 tasks, checksum=100) verified
     - SYNC-TC3: Non-blocking try_lock and boundary behavior verified
     - SYNC-TC4: High-iteration stress test (4,000 increments, zero lost updates) verified
     - SYNC-TC5: Interrupt-safe IrqSpinLock (interrupt masking & state restoration) verified
     - SYNC-TC6: TicketLock FIFO fairness and queue tracking verified
     - SYNC-TC7: Deadlock prevention via non-reentrant try_lock verified
  7. Overall Synchronization test suite confirmation banner (ALL 7 TESTS PASSED).
  8. Live shared-resource synchronization demonstration:
     - Phase A: Unsynchronized shared resource showing data race and lost updates
     - Phase B: Synchronized shared resource (SpinLock<u64>) showing 100% exact updates
     - Lock performance and contention metrics (total acquisitions and contentions)
     - Overall demo success confirmation
  9. Premature QEMU exit detection vs intentional graceful termination.
  10. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55559,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 10 VERIFICATION AUDIT           ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, M6-M9 test suites, demos, and M10 test suite + demo
    time.sleep(6.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone10.ppm"
    png_path = "screenshots/vga_milestone10.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55559), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone10.ppm\n")
            time.sleep(0.5)
            s.sendall(b"quit\n")
            s.close()
            monitor_connected = True
            print("[+] Successfully requested VGA screendump via QEMU monitor socket.")
            break
        except Exception:
            time.sleep(0.5)

    if not monitor_connected:
        print("[-] Warning: Could not connect to QEMU monitor socket for screendump.")

    # Intentional graceful termination of QEMU after collecting evidence
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

    # 1. Verification of Preserved Milestone 6 Tests
    if ">>> MILESTONE 6 KERNEL TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 6 Task Tests: ALL 7 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 6 Task Tests missing or failed!")
        failures.append("Milestone 6 regression detected")

    # 2. Verification of Preserved Milestone 7 Tests
    if ">>> MILESTONE 7 KERNEL TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 7 Ready Queue Tests: ALL 12 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 7 Ready Queue Tests missing or failed!")
        failures.append("Milestone 7 Ready Queue unit tests missing")

    if ">>> MILESTONE 7 REGRESSION TESTS: ALL 13 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 7 Auto-Sync Regression Tests: ALL 13 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 7 Auto-Sync Regression Tests missing or failed!")
        failures.append("Milestone 7 regression tests missing")

    # 3. Verification of Preserved Milestone 8 Tests
    if ">>> MILESTONE 8 SCHEDULER TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 8 Scheduler Tests: ALL 12 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 8 Scheduler Tests missing or failed!")
        failures.append("Milestone 8 Scheduler tests missing")

    if "Demo Success: All 3 tasks executed cooperatively (A: 3/3, B: 3/3, C: 3/3)." in stdout:
        print("[PRESERVATION CHECK] Milestone 8 Cooperative Multi-Task Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 8 Cooperative Multi-Task Demo missing or failed!")
        failures.append("Milestone 8 cooperative demo failed")

    # 4. Verification of Preserved Milestone 9 Tests & Demo
    if ">>> MILESTONE 9 INTERRUPT TESTS: ALL 6 UNIT TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 9 Interrupt Tests: ALL 6 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 9 Interrupt Tests missing or failed!")
        failures.append("Milestone 9 Interrupt tests missing")

    if "Demo Success: All 3 workers completed workloads (A: 3/3, B: 3/3, C: 3/3)." in stdout:
        print("[PRESERVATION CHECK] Milestone 9 Live Preemptive Multi-Task Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 9 Preemptive Multi-Task Demo missing or failed!")
        failures.append("Milestone 9 preemptive demo failed")

    # 5. Verification of 7 In-Kernel Milestone 10 Synchronization Tests
    expected_sync_tests = [
        ("SYNC-TC1: Basic mutual exclusion", "SYNC-TC1: Basic mutual exclusion (2 tasks, val=100) verified."),
        ("SYNC-TC2: Multi-task contention", "SYNC-TC2: Multi-task contention (4 tasks, checksum=100) verified."),
        ("SYNC-TC3: Non-blocking try_lock", "SYNC-TC3: Non-blocking try_lock and boundary behavior verified."),
        ("SYNC-TC4: High-iteration stress", "SYNC-TC4: High-iteration stress test (4000 increments, zero lost updates) verified."),
        ("SYNC-TC5: Interrupt-safe IrqSpinLock", "SYNC-TC5: Interrupt-safe IrqSpinLock (interrupt masking & state restoration) verified."),
        ("SYNC-TC6: TicketLock FIFO fairness", "SYNC-TC6: TicketLock FIFO fairness and queue tracking verified."),
        ("SYNC-TC7: Deadlock defense", "SYNC-TC7: Deadlock prevention via non-reentrant try_lock verified."),
    ]

    print("\n[TEST SUITE 1] In-Kernel Synchronization Programmatic Unit Tests:")
    for name, tag in expected_sync_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel synchronization test failed or missing: {name}")

    if ">>> MILESTONE 10 SYNCHRONIZATION TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Synchronization Test Suite Confirmation: ALL 7 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Synchronization Test Suite confirmation banner missing!")
        failures.append("Synchronization test suite overall summary missing")

    # 6. Verification of Live Shared-Resource Demonstration
    print("\n[TEST SUITE 2] Live Shared-Resource Race Condition vs Synchronization Demonstration:")
    demo_steps = [
        ("Demo Banner", "--- BEGIN DEMO: Shared Resource Race Condition vs Synchronization ---"),
        ("Phase A Banner", "Demo Phase A: Running 3 workers on UNPROTECTED shared resource (no locks)..."),
        ("Phase A Result", "[Phase A Result] Unprotected Counter:"),
        ("Phase A Race Invariant", "[Phase A Invariant] Race condition confirmed: non-atomic access leads to silent data loss!"),
        ("Phase B Banner", "Demo Phase B: Running 3 workers on PROTECTED shared resource (SpinLock<u64>)..."),
        ("Phase B Result", "[Phase B Result] Protected Counter: 3000 / 3000 (Exact Match: 100%)"),
        ("Phase B Invariant", "[Phase B Invariant] Mutual exclusion confirmed: zero lost updates under SpinLock protection."),
        ("Lock Metrics", "[Lock Metrics] Total Acquisitions:"),
        ("Demo Success", "Demo Success: Shared resource problem demonstrated and successfully corrected by SpinLock."),
        ("Idle Loop Banner", "All Milestone 10 tasks and demonstrations completed. Entering idle loop."),
    ]

    last_pos = -1
    for label, pattern in demo_steps:
        pos = stdout.find(pattern, last_pos + 1)
        if pos == -1:
            print(f"  [FAIL] Missing or out of order: {label} (pattern: '{pattern}')")
            failures.append(f"Demonstration step failed or out of order: {label}")
        else:
            print(f"  [PASS] Step: {label}")
            last_pos = pos

    print("\n-----------------------------------------------------------------")
    if not failures:
        print(">>> AUTOMATED ASSERTIONS RESULT: ALL PASS (0 ERRORS) <<<")
    else:
        print(f">>> AUTOMATED ASSERTIONS RESULT: FAILED ({len(failures)} ERRORS) <<<")
        for f in failures:
            print(f"    - {f}")
    print("-----------------------------------------------------------------")

    # 7. Visual VGA Screendump Analysis
    print("\n=================================================================")
    print("     PART B: VISUAL INSPECTION ARTIFACT (MANUAL USER AUDIT)      ")
    print("=================================================================")

    if os.path.exists(ppm_path):
        try:
            from PIL import Image
            import numpy as np

            img = Image.open(ppm_path)
            img.save(png_path)
            print(f"[+] Converted and saved fresh VGA screenshot: {png_path}")

            width, height = img.size
            mode = img.mode
            print(f"[+] Verified screenshot format and dimensions: {width}x{height} pixels (Mode: {mode})")
            if width != 720 or height != 400:
                print(f"[-] Warning: Expected standard 720x400 VGA text resolution, got {width}x{height}")
                failures.append(f"Unexpected VGA resolution: {width}x{height}")

            arr = np.array(img)
            flat_pixels = arr.reshape(-1, arr.shape[2])
            unique_colors, counts = np.unique(flat_pixels, axis=0, return_counts=True)
            print(f"[+] Exactly {len(unique_colors)} unique colors detected on the active VGA screen:")
            for color, count in zip(unique_colors, counts):
                pct = (count / flat_pixels.shape[0]) * 100.0
                if np.array_equal(color, [0, 0, 0]):
                    name = "Black (VGA Background)"
                elif np.array_equal(color, [170, 0, 0]):
                    name = "Red ([ERROR] Log Tag)"
                elif np.array_equal(color, [255, 85, 85]):
                    name = "Light Red / Highlight"
                elif np.array_equal(color, [255, 255, 85]):
                    name = "Yellow ([WARN] Log Tag)"
                elif np.array_equal(color, [85, 255, 85]):
                    name = "Light Green ([INFO] Log Tag)"
                elif np.array_equal(color, [85, 255, 255]):
                    name = "Light Cyan (Kernel Banner & Diagnostics)"
                elif np.array_equal(color, [170, 170, 170]):
                    name = "Light Gray ([DEBUG] Log Tag)"
                elif np.array_equal(color, [255, 255, 255]):
                    name = "White (Standard Log Text Body)"
                else:
                    name = f"RGB({color[0]}, {color[1]}, {color[2]})"
                print(f"    - {name:<38}: {count:>8} pixels ({pct:>5.2f}%)")

            if len(unique_colors) < 2:
                print("[-] Error: VGA screen dump appears completely blank or unrendered!")
                failures.append("Blank VGA framebuffer detected")

        except ImportError:
            print("[-] Warning: PIL or NumPy not installed for PPM->PNG conversion.")
        except Exception as e:
            print(f"[-] Error analyzing VGA screendump: {e}")
            failures.append(f"VGA screendump analysis error: {e}")
    else:
        print(f"[-] Warning: {ppm_path} was not generated.")

    print("\n[MANUAL VISUAL AUDIT GUIDELINES FOR USER]:")
    print("  - Please inspect `screenshots/vga_milestone10.png`.")
    print("  - Confirm that the synchronization test results and shared-resource demo are rendered clearly on screen.")
    print("  - Confirm distinct color codes for [INFO], [WARN], and [DEBUG] log tags.")

    print(f"\n[EXECUTION STATUS] QEMU was intentionally terminated after test completion: True")

    if failures:
        print(f"\n[-] Audit finished with {len(failures)} failures.")
        sys.exit(1)
    else:
        print("\n[+] Audit completed successfully with 0 errors.")
        sys.exit(0)

if __name__ == "__main__":
    run_audit()
