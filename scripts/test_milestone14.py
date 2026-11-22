#!/usr/bin/env python3
"""
Milestone 14 Testing, Debugging, and Telemetry Verification Audit Script
========================================================================
Automated verification audit script covering:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task management tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Preserved Milestone 8 Cooperative Scheduler tests (ALL 12 TESTS PASSED) and live demo.
  5. Preserved Milestone 9 Interrupt unit tests (ALL 6 TESTS PASSED) and live preemption demo.
  6. Preserved Milestone 10 Synchronization unit tests (ALL 7 TESTS PASSED) and live demo.
  7. Preserved Milestone 11 Kernel Memory Allocator tests (ALL 8 TESTS PASSED) and live demo.
  8. Preserved Milestone 12 Process/IPC unit tests (ALL 10 TESTS PASSED) and live demo.
  9. Preserved Milestone 13 User Mode/Syscall tests (ALL 10 TESTS PASSED) and live demo.
  10. Programmatic execution and verification of 6 in-kernel Milestone 14 Testing & Debugging tests:
     - DEBUG-TC1: Minimum 2-Task Configuration Lifecycle & Zero Leakage
     - DEBUG-TC2: Multi-Task Concurrency & Privilege Isolation (4 tasks: 2 kernel + 2 user)
     - DEBUG-TC3: Boundary & Edge Condition Enforcement (Zero-length, capacity bounds, limits)
     - DEBUG-TC4: High-Load Stress & Endurance (100-cycle churn, 64-cycle wrap, IPC saturation, locks)
     - DEBUG-TC5: Comprehensive Failure Injection & Safety Rejection (Syscall, MMU, pointer validation)
     - DEBUG-TC6: Repeatability & Zero-State-Drift Verification (Dual-pass deterministic trial)
  11. Overall Milestone 14 test suite confirmation banner (ALL 6 TESTS PASSED).
  12. Live Milestone 14 Testing & Debugging Demonstration:
     - Step 1: Architectural register baseline capture (CR0, CR2, CR3, CR4, RFLAGS)
     - Step 2: Multi-subsystem trace events recorded into circular ring buffer
     - Step 3: Subsystem telemetry metrics collection and logging
     - Step 4: Circular trace buffer history dump for post-mortem analysis
     - Step 5: Verification of zero state drift, leak-free memory, and kernel health
  13. Premature QEMU exit detection vs intentional graceful termination.
  14. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55564,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 14 VERIFICATION AUDIT          ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, M6-M13 test suites + demos, and M14 test suite + demo
    time.sleep(12.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone14.ppm"
    png_path = "screenshots/vga_milestone14.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55564), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone14.ppm\n")
            time.sleep(0.5)
            s.sendall(b"quit\n")
            s.close()
            monitor_connected = True
            print("[+] Successfully requested VGA screendump via QEMU monitor socket.")
            break
        except Exception:
            time.sleep(0.5)

    if not monitor_connected:
        print("[-] Warning: Could not connect to QEMU monitor socket on 127.0.0.1:55564.")

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
    print("-----------------------------------------------------------------\n")

    failures = []

    # 1. Verification of Milestone 6 Preservation
    if ">>> MILESTONE 6 KERNEL TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 6 Task Tests: ALL 7 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 6 Task tests missing or failed!")
        failures.append("Milestone 6 tests failed")

    # 2. Verification of Milestone 7 Preservation
    if ">>> MILESTONE 7 KERNEL TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 7 Ready Queue Tests: ALL 12 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 7 Ready Queue tests missing or failed!")
        failures.append("Milestone 7 tests failed")

    if ">>> MILESTONE 7 REGRESSION TESTS: ALL 13 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 7 Auto-Sync Regression Tests: ALL 13 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 7 Auto-Sync Regression tests missing or failed!")
        failures.append("Milestone 7 auto-sync tests failed")

    # 3. Verification of Milestone 8 Preservation
    if ">>> MILESTONE 8 SCHEDULER TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 8 Scheduler Tests: ALL 12 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 8 Scheduler tests missing or failed!")
        failures.append("Milestone 8 tests failed")

    if "Demo Success: All 3 tasks executed cooperatively (A: 3/3, B: 3/3, C: 3/3)." in stdout:
        print("[PRESERVATION CHECK] Milestone 8 Cooperative Multi-Task Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 8 Cooperative Multi-Task Demo missing or failed!")
        failures.append("Milestone 8 demo failed")

    # 4. Verification of Milestone 9 Preservation
    if ">>> MILESTONE 9 INTERRUPT TESTS: ALL 6 UNIT TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 9 Interrupt Tests: ALL 6 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 9 Interrupt tests missing or failed!")
        failures.append("Milestone 9 tests failed")

    if "Live timer-driven preemptive execution demonstration verified successfully." in stdout:
        print("[PRESERVATION CHECK] Milestone 9 Live Preemptive Multi-Task Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 9 Preemptive Multi-Task Demo missing or failed!")
        failures.append("Milestone 9 preemptive demo failed")

    # 5. Verification of Milestone 10 Preservation
    if ">>> MILESTONE 10 SYNCHRONIZATION TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 10 Synchronization Tests: ALL 7 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 10 Synchronization tests missing or failed!")
        failures.append("Milestone 10 tests failed")

    if "Live shared-resource synchronization demonstration verified successfully." in stdout:
        print("[PRESERVATION CHECK] Milestone 10 Live Synchronization Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 10 Synchronization demo missing or failed!")
        failures.append("Milestone 10 demo failed")

    # 6. Verification of Milestone 11 Preservation
    if ">>> MILESTONE 11 MEMORY TESTS: ALL 8 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 11 Memory Tests: ALL 8 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 11 Memory tests missing or failed!")
        failures.append("Milestone 11 tests failed")

    if "Demo Success: Kernel heap allocation, dynamic structures, and memory recycling verified." in stdout:
        print("[PRESERVATION CHECK] Milestone 11 Live Heap Allocation Demo: PASSED.")
    else:
        print("[-] FAIL: Milestone 11 Live Heap Demo missing or failed!")
        failures.append("Milestone 11 demo failed")

    # 7. Verification of Milestone 12 Preservation
    if ">>> MILESTONE 12 KERNEL TESTS: ALL 10 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 12 Process & IPC Tests: ALL 10 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 12 Process tests missing or failed!")
        failures.append("Milestone 12 tests failed")

    # 8. Verification of Milestone 13 Preservation
    if ">>> MILESTONE 13 KERNEL TESTS: ALL 10 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 13 User Mode & Syscall Tests: ALL 10 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 13 User Mode tests missing or failed!")
        failures.append("Milestone 13 tests failed")

    if "M13-Demo Step 4: User Mode application completed cleanly via SYS_EXIT with code 0." in stdout:
        print("[PRESERVATION CHECK] Milestone 13 User Mode Demonstration: PASSED.")
    else:
        print("[-] FAIL: Milestone 13 Demo missing or failed!")
        failures.append("Milestone 13 demo failed")

    # 9. Verification of 6 In-Kernel Milestone 14 Testing & Debugging Tests
    expected_m14_tests = [
        ("DEBUG-TC1: Minimum 2-Task Configuration Lifecycle", "DEBUG-TC1: Minimum 2-task configuration lifecycle and zero resource leakage verified."),
        ("DEBUG-TC2: Multi-Task Concurrency & Privilege Isolation", "DEBUG-TC2: Multi-task concurrency and hardware address space isolation (4 tasks) verified."),
        ("DEBUG-TC3: Boundary & Limit Enforcement", "DEBUG-TC3: Boundary conditions (0-length, empty queue, capacity bounds) strictly enforced."),
        ("DEBUG-TC4: High-Load Stress & Endurance", "DEBUG-TC4: High-load stress (100 alloc churn, 64 queue wraps, IPC saturation, sequential lock stress) verified."),
        ("DEBUG-TC5: Failure Injection & Safety Rejection", "DEBUG-TC5: Failure injection (ENOSYS, EFAULT, EINVAL, illegal transitions, duplicates, Ring 3 int 0x80) safely rejected."),
        ("DEBUG-TC6: Repeatability & Zero-State-Drift", "DEBUG-TC6: Repeatability and zero state drift verified across consecutive runs (Delta = 0)."),
    ]

    print("\n[TEST SUITE 1] In-Kernel Milestone 14 Programmatic Test Cases (DEBUG-TC1 to DEBUG-TC6):")
    for name, tag in expected_m14_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel Milestone 14 test failed or missing: {name}")

    if ">>> MILESTONE 14 KERNEL TESTS: ALL 6 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Milestone 14 Test Suite Confirmation: ALL 6 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Milestone 14 Test Suite confirmation banner missing!")
        failures.append("Milestone 14 test suite overall summary missing")

    # 10. Verification of Live Milestone 14 Testing & Debugging Demonstration
    print("\n[TEST SUITE 2] Live Milestone 14 Testing & Debugging Demonstration:")
    demo_steps = [
        ("Demo Banner", "--- BEGIN DEMO: Milestone 14 Comprehensive Testing & Debugging Demonstration ---"),
        ("Step 1: Register Baseline", "M14-Demo Step 1: Captured architectural register baseline (CR0, CR2, CR3, CR4, RFLAGS)."),
        ("Step 2: Trace Events", "M14-Demo Step 2: Recorded multi-subsystem trace events into circular ring buffer."),
        ("Step 3: Telemetry Collection", "M14-Demo Step 3: Collected and logged live subsystem telemetry metrics."),
        ("Step 4: Trace Buffer Dump", "M14-Demo Step 4: Circular trace buffer dumped successfully for post-mortem analysis."),
        ("Step 5: Zero State Drift", "M14-Demo Step 5: Verified zero state drift, leak-free memory, and kernel health."),
        ("Idle Loop Banner", "All Milestone 14 tasks, tests, and demonstrations completed successfully. Entering idle loop."),
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

    # 11. Visual VGA Screendump Analysis
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
                    name = "Light Cyan (Banner Border)"
                elif np.array_equal(color, [170, 170, 170]):
                    name = "Light Gray ([DEBUG] Log Tag)"
                elif np.array_equal(color, [255, 255, 255]):
                    name = "White (Standard Log Text Body)"
                else:
                    name = f"RGB({color[0]}, {color[1]}, {color[2]})"
                print(f"    - {name:<38}: {count:>8} pixels ({pct:>5.2f}%)")

            print("\n[MANUAL VISUAL AUDIT GUIDELINES FOR USER]:")
            print(f"  - Please inspect `{png_path}`.")
            print("  - Confirm that the Milestone 14 test results and demo are rendered clearly on screen.")
            print("  - Confirm distinct color codes for [INFO], [WARN], and [DEBUG] log tags.")

        except Exception as e:
            print(f"[-] Warning: Image processing failed: {e}")
            failures.append(f"Screenshot conversion error: {e}")
    else:
        print(f"[-] Error: VGA screendump file `{ppm_path}` not found!")
        failures.append("Missing VGA screendump file")

    print(f"\n[EXECUTION STATUS] QEMU was intentionally terminated after test completion: True")
    if failures:
        print(f"\n[-] Audit completed with {len(failures)} failures.")
        sys.exit(1)
    else:
        print("\n[+] Audit completed successfully with 0 errors.")
        sys.exit(0)

if __name__ == "__main__":
    run_audit()
