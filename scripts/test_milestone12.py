#!/usr/bin/env python3
"""
Milestone 12 Process/Thread Specialization & IPC Verification Audit Script (Paper R6)
=====================================================================================
Automated verification audit script covering:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task management tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Preserved Milestone 8 Cooperative Scheduler tests (ALL 12 TESTS PASSED) and live demo.
  5. Preserved Milestone 9 Interrupt unit tests (ALL 6 TESTS PASSED) and live preemption demo.
  6. Preserved Milestone 10 Synchronization unit tests (ALL 7 TESTS PASSED) and live demo.
  7. Preserved Milestone 11 Kernel Memory Allocator tests (ALL 8 TESTS PASSED) and live demo.
  8. Programmatic execution and verification of 10 in-kernel Milestone 12 Process/IPC tests:
     - PROC-TC1: Process creation, PID generation, and descriptor verification
     - PROC-TC2: Thread attachment and capacity bound (4 threads/process)
     - PROC-TC3: Generation persistence across slot reuse and stale PID rejection
     - PROC-TC4: Logical 16 KiB heap partition allocation and tracking
     - PROC-TC5: IPC message exchange, strict FIFO order, and mailbox capacity bound
     - PROC-TC6: IPC receiver safe rejection of messages from reaped stale sender
     - PROC-TC7: SleepQueue tick delta scheduling and automatic task wakeup
     - PROC-TC8: ReadyQueue two-phase reservation (commit, cancel, timeout)
     - PROC-TC9: Deferred task reclamation (reap_terminated_tasks cleans Terminated slots)
     - PROC-TC10: Fail-closed TaskId generation
  9. Overall Milestone 12 test suite confirmation banner (ALL 10 TESTS PASSED).
  10. Verification of R6 Research Benchmark Suite (5 Hypotheses):
     - H1: Thread vs Process Creation Latency (RDTSC cycle count)
     - H2: CPU-Bound Execution Scalability (Round-Robin compute parity)
     - H3: I/O & Sleep Responsiveness (Sleep wakeup resolution & IPC latency)
     - H4: Shared Memory vs IPC Copying (Data throughput & copying overhead)
     - H5: Memory Footprint (Per-thread vs per-process allocation accounting)
     - R6 Benchmark Suite confirmation banner (ALL 5 HYPOTHESES EVALUATED).
  11. Live Process & IPC demonstration:
     - Step 1: Process creation (Process-Alpha, Process-Beta)
     - Step 2: Thread attachment to process
     - Step 3: IPC message transmission
     - Step 4: IPC message reception and payload verification
     - Step 5: Clean thread termination and process reaping
  12. Premature QEMU exit detection vs intentional graceful termination.
  13. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55562,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 12 VERIFICATION AUDIT          ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, M6-M11 test suites + demos, and M12 test suite + benchmarks + demo
    time.sleep(8.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone12.ppm"
    png_path = "screenshots/vga_milestone12.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55562), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone12.ppm\n")
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
    print("-----------------------------------------------------------------")

    failures = []

    print("\n=================================================================")
    print("       PART A: AUTOMATED CONFORMANCE ASSERTIONS (PROGRAMMATIC)   ")
    print("=================================================================")

    # 1. Verification of Milestone 6 Preservation
    if ">>> MILESTONE 6 KERNEL TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("[PRESERVATION CHECK] Milestone 6 Task Tests: ALL 7 TESTS PASSED.")
    else:
        print("[-] FAIL: Milestone 6 Task Tests missing or failed!")
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
        print("[-] FAIL: Milestone 7 Auto-Sync tests missing or failed!")
        failures.append("Milestone 7 Auto-Sync tests failed")

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

    # 7. Verification of 10 In-Kernel Milestone 12 Process/IPC Tests
    expected_m12_tests = [
        ("PROC-TC1: Process creation, PID, and descriptor", "PROC-TC1: Process creation, PID"),
        ("PROC-TC2: Thread attachment and capacity bound", "PROC-TC2: Thread attachment and capacity bound"),
        ("PROC-TC3: Generation persistence & stale PID rejection", "PROC-TC3: Generation persistence"),
        ("PROC-TC4: Logical 16 KiB heap partition allocation", "PROC-TC4: Logical 16 KiB heap partition allocation"),
        ("PROC-TC5: IPC message exchange, FIFO order & capacity", "PROC-TC5: IPC message exchange, strict FIFO order, and mailbox capacity verified."),
        ("PROC-TC6: IPC receiver stale sender rejection", "PROC-TC6: IPC receiver safely rejects messages from reaped stale sender."),
        ("PROC-TC7: SleepQueue tick delta scheduling & wakeup", "PROC-TC7: SleepQueue tick delta scheduling and automatic task wakeup verified."),
        ("PROC-TC8: ReadyQueue two-phase reservation", "PROC-TC8: ReadyQueue two-phase reservation (commit, cancel, timeout) verified."),
        ("PROC-TC9: Deferred task reclamation", "PROC-TC9: Deferred task reclamation successfully reclaimed"),
        ("PROC-TC10: Fail-closed TaskId generation", "PROC-TC10: Fail-closed TaskId generation verified"),
    ]

    print("\n[TEST SUITE 1] In-Kernel Milestone 12 Programmatic Unit Tests:")
    for name, tag in expected_m12_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel Milestone 12 test failed or missing: {name}")

    if ">>> MILESTONE 12 KERNEL TESTS: ALL 10 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Milestone 12 Test Suite Confirmation: ALL 10 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Milestone 12 Test Suite confirmation banner missing!")
        failures.append("Milestone 12 test suite overall summary missing")

    # 8. Verification of R6 Research Benchmark Suite
    expected_benchmarks = [
        ("H1 Benchmark: Thread vs Process Creation Cost", "[H1 RESULT] Creation Cost Ratio:"),
        ("H2 Benchmark: CPU-Bound Execution Scalability", "[H2 RESULT] Single-core round-robin yields identical compute parity."),
        ("H3 Benchmark: I/O & Sleep Responsiveness", "[H3 RESULT] Sleep Wakeup Resolution:"),
        ("H4 Benchmark: Shared Memory vs IPC Copying", "[H4 RESULT] IPC Copy Overhead:"),
        ("H5 Benchmark: Memory Footprint Accounting", "[H5 RESULT] Memory Footprint Ratio:"),
    ]

    print("\n[TEST SUITE 2] R6 Research Benchmark Suite (5 Hypotheses):")
    for name, tag in expected_benchmarks:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"R6 benchmark failed or missing: {name}")

    if ">>> MILESTONE 12 R6 BENCHMARKS: ALL 5 HYPOTHESES EVALUATED <<<" in stdout:
        print("  [PASS] Overall R6 Benchmark Confirmation: ALL 5 HYPOTHESES EVALUATED.")
    else:
        print("  [FAIL] Overall R6 Benchmark confirmation banner missing!")
        failures.append("R6 benchmark suite overall summary missing")

    # 9. Verification of Live Milestone 12 Process & IPC Demonstration
    print("\n[TEST SUITE 3] Live Milestone 12 Process & IPC Demonstration:")
    demo_steps = [
        ("Demo Banner", "--- BEGIN DEMO: Milestone 12 Process & IPC Demonstration ---"),
        ("Step 1 Result", "M12-Demo Step 1: Created"),
        ("Step 2 Result", "M12-Demo Step 2: Attached thread"),
        ("Step 3 Result", "M12-Demo Step 3: Sent IPC message"),
        ("Step 4 Result", "M12-Demo Step 4: Received message at"),
        ("Step 5 Result", "M12-Demo Step 5: Cleaned up threads and reaped processes cleanly."),
        ("Idle Loop Banner", "All Milestone 12 tasks, benchmarks, and demonstrations completed successfully. Entering idle loop."),
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

    # 10. Visual VGA Screendump Analysis
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
            print("  - Confirm that the Milestone 12 test results, benchmarks, and demo are rendered clearly on screen.")
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
