#!/usr/bin/env python3
"""
Milestone 8 Cooperative Round Robin Scheduler Verification Audit Script
========================================================================
Automated test suite verifying:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Programmatic execution and verification of 12 in-kernel Scheduler tests:
     - SCHED-TC1: Scheduler initialization state verified
     - SCHED-TC2: Selecting first Ready task in queue verified
     - SCHED-TC3: FIFO selection order of multiple tasks verified
     - SCHED-TC4: Cooperative yield re-queues yielded task at tail
     - SCHED-TC5: Round Robin cyclic fairness across multiple yields verified
     - SCHED-TC6: Empty ReadyQueue handled safely (solo task retains CPU)
     - SCHED-TC7: Blocked task excluded from queue and never scheduled
     - SCHED-TC8: Terminated task purged and never re-queued
     - SCHED-TC9: Duplicate ReadyQueue entries prevented during scheduling
     - SCHED-TC10: State consistency between TaskManager and ReadyQueue strictly verified
     - SCHED-TC11: Task registration vs ReadyQueue scheduling eligibility verified
     - SCHED-TC12: Stack pointer validation boundary conditions (7 cases) verified
  5. Overall Scheduler test suite confirmation banner (ALL 12 TESTS PASSED).
  6. Live multi-task cooperative execution demonstration in strict sequential order:
     - 3 independent tasks (Worker-A, Worker-B, Worker-C) with private 4 KiB stacks.
     - Interleaved execution: A1 -> B1 -> C1 -> A2 -> B2 -> C2 -> A3 -> B3 -> C3.
     - Voluntary yield via sys_yield() with real CPU register/stack context switching.
     - Safe task completion via sys_exit() and graceful return to kernel main thread.
     - Atomic verification of iteration counts (3/3 each, 9 switches total).
  7. Premature QEMU exit detection vs intentional termination.
  8. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55558,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 8 VERIFICATION AUDIT            ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, M6/M7 test suites, M8 unit tests, and live demo
    time.sleep(5.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone8.ppm"
    png_path = "screenshots/vga_milestone8.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55558), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone8.ppm\n")
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

    # 3. Verification of 12 In-Kernel Milestone 8 Scheduler Tests
    expected_sched_tests = [
        ("SCHED-TC1: Scheduler initialization state", "SCHED-TC1: Scheduler initialization state verified"),
        ("SCHED-TC2: Select first Ready task", "SCHED-TC2: Selecting first Ready task in queue verified"),
        ("SCHED-TC3: FIFO selection order", "SCHED-TC3: FIFO selection order (T1 -> T2 -> T3) verified"),
        ("SCHED-TC4: Cooperative yield & tail re-queue", "SCHED-TC4: Cooperative yield re-queues yielded task at queue tail"),
        ("SCHED-TC5: Round Robin cyclic fairness", "SCHED-TC5: Round Robin cyclic fairness across 6 yields verified"),
        ("SCHED-TC6: Empty ReadyQueue safety", "SCHED-TC6: Empty ReadyQueue handled safely; solo task retains CPU"),
        ("SCHED-TC7: Blocked task never scheduled", "SCHED-TC7: Blocked task is excluded from ReadyQueue and never scheduled"),
        ("SCHED-TC8: Terminated task never requeued", "SCHED-TC8: Terminated task is purged and never re-queued"),
        ("SCHED-TC9: Duplicate queue prevention", "SCHED-TC9: Duplicate ReadyQueue entries prevented during scheduler transitions"),
        ("SCHED-TC10: TM & RQ state consistency", "SCHED-TC10: State consistency between TaskManager and ReadyQueue strictly verified"),
        ("SCHED-TC11: Task registration vs eligibility", "SCHED-TC11: Task registration vs ReadyQueue scheduling eligibility verified"),
        ("SCHED-TC12: Stack validation boundaries", "SCHED-TC12: Stack pointer validation boundary conditions (7 cases) verified"),
    ]

    print("\n[TEST SUITE 1] In-Kernel Cooperative Scheduler Programmatic Unit Tests:")
    for name, tag in expected_sched_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel scheduler test failed or missing: {name}")

    if ">>> MILESTONE 8 SCHEDULER TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Scheduler Test Suite Confirmation: ALL 12 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Scheduler Test Suite confirmation banner missing!")
        failures.append("Scheduler test suite overall summary missing")

    # 4. Verification of Live Cooperative Task Switching Demonstration in Strict Sequential Order
    print("\n[TEST SUITE 2] Live Cooperative Multi-Task Demonstration (Sequential Order Check):")
    demo_ordered_steps = [
        ("Demo Banner", "--- BEGIN DEMO: Live Cooperative Round Robin Execution ---"),
        ("Demo Task Registration", "Demo Setup: Registered tasks"),
        ("Demo Queue Order", "Demo Setup: Enqueued tasks in FIFO order:"),
        ("Start Scheduler", "Starting cooperative Round Robin scheduler..."),
        ("Worker-A Iter 1", "[Task A] Running iteration 1/3 - yielding CPU voluntarily."),
        ("Worker-B Iter 1", "[Task B] Running iteration 1/3 - yielding CPU voluntarily."),
        ("Worker-C Iter 1", "[Task C] Running iteration 1/3 - yielding CPU voluntarily."),
        ("Worker-A Iter 2", "[Task A] Running iteration 2/3 - yielding CPU voluntarily."),
        ("Worker-B Iter 2", "[Task B] Running iteration 2/3 - yielding CPU voluntarily."),
        ("Worker-C Iter 2", "[Task C] Running iteration 2/3 - yielding CPU voluntarily."),
        ("Worker-A Iter 3", "[Task A] Running iteration 3/3 - yielding CPU voluntarily."),
        ("Worker-B Iter 3", "[Task B] Running iteration 3/3 - yielding CPU voluntarily."),
        ("Worker-C Iter 3", "[Task C] Running iteration 3/3 - yielding CPU voluntarily."),
        ("Worker-A Exit", "[Task A] Completed all 3 iterations - exiting."),
        ("Worker-B Exit", "[Task B] Completed all 3 iterations - exiting."),
        ("Worker-C Exit", "[Task C] Completed all 3 iterations - exiting."),
        ("Return to Main Thread", "Returned to kernel main thread! Verifying execution counts..."),
        ("Demo Success Verification", "Demo Success: All 3 tasks executed cooperatively (A: 3/3, B: 3/3, C: 3/3)."),
        ("Fairness Confirmation", "Actual CPU context switching and Round Robin fairness confirmed."),
        ("Stack Diagnostic Banner", "Hardware Stack Diagnostic Evidence:"),
        ("Worker-A RSP Check", "Worker-A    RSP:"),
        ("Worker-B RSP Check", "Worker-B    RSP:"),
        ("Worker-C RSP Check", "Worker-C    RSP:"),
        ("Stack Diagnostic Pass", "Stack Diagnostic: PASS - Confirmed distinct, isolated 4 KiB hardware stacks."),
        ("Overall Demo Success", "Live cooperative task execution demonstration verified successfully."),
        ("Idle Loop Message", "All Milestone 8 tasks and demonstrations completed. Entering idle loop."),
    ]

    last_pos = -1
    for label, pattern in demo_ordered_steps:
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

        try:
            img = Image.open(ppm_path)
            img.save(png_path)
            print(f"[+] Converted and saved fresh VGA screenshot: {png_path}")
            print(f"[+] Verified screenshot format and dimensions: {img.width}x{img.height} pixels (Mode: {img.mode})")

            arr = np.array(img)
            unique_colors, counts = np.unique(arr.reshape(-1, arr.shape[2]), axis=0, return_counts=True)
            print(f"[+] Exactly {len(unique_colors)} unique colors detected on the active VGA screen:")

            color_names = {
                (0, 0, 0): "Black (VGA Background)",
                (85, 255, 255): "Light Cyan (INFO Tag & Header Banner)",
                (255, 87, 87): "Light Red (ERROR Tag)",
                (255, 255, 87): "Yellow (WARN Tag)",
                (168, 168, 168): "Light Gray (DEBUG Tag)",
                (255, 255, 255): "White (Standard Log Text Body)",
            }

            for color, count in zip(unique_colors, counts):
                rgb = tuple(int(c) for c in color[:3])
                label = color_names.get(rgb, f"RGB{rgb}")
                pct = (count / (img.width * img.height)) * 100
                print(f"    - {label:<38}: {count:>6} pixels ({pct:5.2f}%)")

            print("\n[MANUAL VISUAL AUDIT GUIDELINES FOR USER]:")
            print("  - Please inspect `screenshots/vga_milestone8.png`.")
            print("  - Confirm that the cooperative Round Robin demonstration and iteration yields are rendered clearly on screen.")
            print("  - Confirm distinct color codes for [INFO], [WARN], and [DEBUG] log tags.")
        except Exception as e:
            print(f"[-] ERROR: Failed to open or inspect screenshot: {e}")
            failures.append(f"Screenshot open/inspect failed: {e}")
    else:
        print("[-] Screenshot PPM file not found or empty.")
        failures.append("Screenshot PPM missing")

    print(f"\n[EXECUTION STATUS] QEMU was intentionally terminated after test completion: {proc.returncode is not None}")
    if failures:
        print(f"[-] Audit completed with {len(failures)} failures.")
        sys.exit(1)
    else:
        print("[+] Audit completed successfully with 0 errors.")
        sys.exit(0)

if __name__ == "__main__":
    run_audit()
