#!/usr/bin/env python3
"""
Milestone 9 Timer Interrupts & Preemptive Scheduling Verification Audit Script
==============================================================================
Automated verification audit script covering:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task management tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Preserved Milestone 8 Cooperative Scheduler tests (ALL 12 TESTS PASSED) and live demo.
  5. Programmatic execution and verification of 6 in-kernel Milestone 9 Interrupt tests:
     - INT-TC1: PIT 100 Hz divisor calculation (11932 = 0x2E9C, err < 0.002%) verified
     - INT-TC2: 8259 PIC remapping (Master: 32, Slave: 40, IRQ0: 32) verified
     - INT-TC3: IDT entry 32 points to valid timer handler
     - INT-TC4: Hardware timer tick progression verified
     - INT-TC5: Scheduler preemption state transition verified
     - INT-TC6: Ready queue consistency under preemption verified
  6. Overall Interrupt test suite confirmation banner (ALL 6 UNIT TESTS PASSED).
  7. Live multi-task preemptive execution demonstration:
     - 3 independent CPU-bound worker tasks (Preempt-A, Preempt-B, Preempt-C) with private 4 KiB stacks.
     - Workers execute CPU-bound computation loops WITHOUT calling sys_yield().
     - PIT timer interrupts at 100 Hz trigger preemption and context switching.
     - Interleaved execution and resumption verified through hardware timer IRQs.
     - Clean task completion via sys_exit() and graceful return to kernel main thread.
     - Hardware stack diagnostics confirming isolated 4 KiB stacks during interrupts.
  8. Premature QEMU exit detection vs intentional graceful termination.
  9. Visual VGA screendump verification and color palette analysis.
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
    print("        RUST MICRO-OS: MILESTONE 9 VERIFICATION AUDIT            ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, M6/M7/M8 test suites, M9 unit tests, and live demo
    time.sleep(5.5)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone9.ppm"
    png_path = "screenshots/vga_milestone9.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55559), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone9.ppm\n")
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

    # 4. Verification of 6 In-Kernel Milestone 9 Interrupt Tests
    expected_int_tests = [
        ("INT-TC1: PIT divisor calculation", "INT-TC1: PIT 100 Hz divisor calculation (11932 = 0x2E9C, err < 0.002%) verified"),
        ("INT-TC2: PIC remapping & vector config", "INT-TC2: 8259 PIC remapping (Master: 32, Slave: 40, IRQ0: 32) verified"),
        ("INT-TC3: IDT timer vector registration", "INT-TC3: IDT entry 32 points to valid timer handler"),
        ("INT-TC4: Timer tick progression", "INT-TC4: Hardware timer tick progression"),
        ("INT-TC5: Scheduler preemption state", "INT-TC5: Scheduler preemption state transition (Running->Ready at tail) verified"),
        ("INT-TC6: Ready queue preemption consistency", "INT-TC6: Ready queue consistency under preemption (blocked/term excluded) verified"),
    ]

    print("\n[TEST SUITE 1] In-Kernel Interrupt & Timer Programmatic Unit Tests:")
    for name, tag in expected_int_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel interrupt test failed or missing: {name}")

    if ">>> MILESTONE 9 INTERRUPT TESTS: ALL 6 UNIT TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Interrupt Test Suite Confirmation: ALL 6 UNIT TESTS PASSED.")
    else:
        print("  [FAIL] Overall Interrupt Test Suite confirmation banner missing!")
        failures.append("Interrupt test suite overall summary missing")

    # 5. Verification of Live Preemptive Multi-Task Demonstration
    print("\n[TEST SUITE 2] Live Timer-Driven Preemptive Multi-Task Demonstration:")

    # (a) Verify all three independent workers started and completed their workloads
    worker_checks = [
        ("Worker-A Started", "[Preempt Worker-A] Started CPU-bound computation."),
        ("Worker-B Started", "[Preempt Worker-B] Started CPU-bound computation."),
        ("Worker-C Started", "[Preempt Worker-C] Started CPU-bound computation."),
        ("Worker-A Completed", "[Preempt Worker-A] Completed all 3 compute phases - exiting."),
        ("Worker-B Completed", "[Preempt Worker-B] Completed all 3 compute phases - exiting."),
        ("Worker-C Completed", "[Preempt Worker-C] Completed all 3 compute phases - exiting."),
    ]
    for label, pattern in worker_checks:
        if pattern in stdout:
            print(f"  [PASS] {label}")
        else:
            print(f"  [FAIL] Missing expected worker pattern: {pattern}")
            failures.append(f"Worker check failed: {label}")

    # (b) Verify hardware timer preemption IRQ messages exist in serial log
    irq_count = stdout.count("[TIMER IRQ #")
    if irq_count >= 6:
        print(f"  [PASS] Timer Interrupt Preemptions Verified: {irq_count} preemption events recorded.")
    else:
        print(f"  [FAIL] Insufficient timer preemption events detected (found {irq_count}, expected >= 6)")
        failures.append(f"Insufficient timer preemption events ({irq_count})")

    # (c) Verify strict sequential execution order of major lifecycle milestones
    demo_ordered_steps = [
        ("Preempt Demo Banner", "--- BEGIN DEMO: Live Preemptive Round Robin Execution (100 Hz Timer) ---"),
        ("Preempt Task Registration", "Demo Setup: Registered preemptive tasks"),
        ("Preempt Queue Order", "Demo Setup: Enqueued tasks in FIFO order:"),
        ("Start Preemptive Scheduler", "Starting timer-driven preemptive Round Robin scheduler..."),
        ("Return to Main Thread", "Returned to kernel main thread! Verifying preemptive execution..."),
        ("Demo Success Verification", "Demo Success: All 3 workers completed workloads (A: 3/3, B: 3/3, C: 3/3)."),
        ("Hardware Preemption Confirmed", "Hardware timer preemption confirmed:"),
        ("Stack Diagnostic Banner", "Hardware Stack Diagnostic Evidence (Preemptive Tasks):"),
        ("Preempt-A RSP Check", "Preempt-A   RSP:"),
        ("Preempt-B RSP Check", "Preempt-B   RSP:"),
        ("Preempt-C RSP Check", "Preempt-C   RSP:"),
        ("Stack Diagnostic Pass", "Stack Diagnostic: PASS - Confirmed distinct, isolated 4 KiB stacks during timer interrupts."),
        ("Hardware Frame Verification", "Hardware Frame Verification: PASS - Confirmed RIP="),
        ("Overall Demo Success", "Live timer-driven preemptive execution demonstration verified successfully."),
        ("Idle Loop Message", "All Milestone 9 tasks and demonstrations completed. Entering idle loop."),
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
        print(f">>> AUTOMATED ASSERTIONS RESULT: FAILED ({len(failures)} ERRORS) <<<")
        for f in failures:
            print(f"    - {f}")
    print("-----------------------------------------------------------------")

    # 6. Visual VGA Screendump Analysis
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
    print("  - Please inspect `screenshots/vga_milestone9.png`.")
    print("  - Confirm that the timer-driven preemptive execution demonstration and interrupt ticks are rendered clearly on screen.")
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
