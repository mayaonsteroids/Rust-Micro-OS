#!/usr/bin/env python3
"""
Milestone 11 Kernel Memory Allocator Verification Audit Script
==============================================================
Automated verification audit script covering:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task management tests (ALL 7 TESTS PASSED).
  3. Preserved Milestone 7 Ready Queue unit & regression tests (ALL 12 + ALL 13 PASSED).
  4. Preserved Milestone 8 Cooperative Scheduler tests (ALL 12 TESTS PASSED) and live demo.
  5. Preserved Milestone 9 Interrupt unit tests (ALL 6 TESTS PASSED) and live preemption demo.
  6. Preserved Milestone 10 Synchronization unit tests (ALL 7 TESTS PASSED) and live demo.
  7. Verification of Kernel Memory Subsystem initialization:
     - BootInfoFrameAllocator physical RAM detection
     - OffsetPageTable initialization
     - Heap page mapping (256 KiB)
  8. Programmatic execution and verification of 8 in-kernel Milestone 11 Memory tests:
     - MEM-TC1: Basic Box Allocation & Dereference
     - MEM-TC2: Dynamic Vector Growth & Reallocation (1,000 Elements)
     - MEM-TC3: Multi-Alignment Verification (1, 4, 8, 16, 32, 64 bytes)
     - MEM-TC4: Memory Deallocation & Free-List Node Reuse
     - MEM-TC5: Safe Allocation Failure & Out-of-Memory Handling
     - MEM-TC6: Interleaved High-Iteration Stress Test
     - MEM-TC7: Interrupt-Safe Allocator Locking Invariant
     - MEM-TC8: Educational Allocator Comparison (Bump vs Free-List)
  9. Overall Memory test suite confirmation banner (ALL 8 TESTS PASSED).
  10. Live kernel heap allocation & dynamic data structures demonstration:
     - Baseline metrics
     - Step 1: Dynamic Vector of Task Descriptors
     - Step 2: Boxed Recursive Tree Nodes
     - Step 3: Deallocation & Memory Reclamation
     - Overall demo success confirmation
  11. Premature QEMU exit detection vs intentional graceful termination.
  12. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55560,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 11 VERIFICATION AUDIT           ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, M6-M10 test suites + demos, and M11 test suite + demo
    time.sleep(7.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone11.ppm"
    png_path = "screenshots/vga_milestone11.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55560), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone11.ppm\n")
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

    # 6. Verification of Kernel Memory Subsystem Initialization
    print("\n[SUBSYSTEM INITIALIZATION] Kernel Memory & Heap Subsystem:")
    init_checks = [
        ("Memory Banner", "INITIALIZING KERNEL MEMORY SUBSYSTEM"),
        ("Physical RAM Detection", "[MEMORY] Physical RAM:"),
        ("Virtual Mapping Offset", "[MEMORY] Physical memory virtual mapping offset:"),
        ("Heap Page Mapping", "[MEMORY] Kernel Heap successfully mapped: [0x444444440000 .. 0x444444480000] (256 KiB)."),
        ("Physical Frame Allocation", "[MEMORY] Physical frames allocated for heap:"),
    ]
    for label, pattern in init_checks:
        if pattern in stdout:
            print(f"  [PASS] {label}")
        else:
            print(f"  [FAIL] {label} - Missing pattern: '{pattern}'")
            failures.append(f"Memory initialization check failed: {label}")

    # 7. Verification of 8 In-Kernel Milestone 11 Memory Tests
    expected_mem_tests = [
        ("MEM-TC1: Basic Box allocation", "MEM-TC1: Basic Box allocation verified"),
        ("MEM-TC2: Dynamic vector growth", "MEM-TC2: Dynamic vector growth verified (1000 elements, sum=499500)."),
        ("MEM-TC3: Multi-alignment verification", "MEM-TC3: Multi-alignment (1, 4, 8, 16, 32, 64 bytes) verified."),
        ("MEM-TC4: Deallocation & memory reuse", "MEM-TC4: Deallocation and free-list memory reuse verified"),
        ("MEM-TC5: Safe allocation failure handling", "MEM-TC5: Safe allocation failure, lifecycle states & re-init rejection verified."),
        ("MEM-TC6: Interleaved stress test", "MEM-TC6: Interleaved stress test"),
        ("MEM-TC7: Interrupt-safe allocator locking", "MEM-TC7: Interrupt-safe allocator locking invariant"),
        ("MEM-TC8: Educational allocator comparison", "MEM-TC8: Educational allocator comparison (BumpAllocator tracking, double-free safety & reset) verified."),
    ]

    print("\n[TEST SUITE 1] In-Kernel Memory Allocator Programmatic Unit Tests:")
    for name, tag in expected_mem_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel memory test failed or missing: {name}")

    if ">>> MILESTONE 11 MEMORY TESTS: ALL 8 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Memory Test Suite Confirmation: ALL 8 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Memory Test Suite confirmation banner missing!")
        failures.append("Memory test suite overall summary missing")

    # 8. Verification of Live Heap Allocation & Dynamic Data Structures Demonstration
    print("\n[TEST SUITE 2] Live Heap Allocation & Dynamic Data Structures Demonstration:")
    demo_steps = [
        ("Demo Banner", "--- BEGIN DEMO: Kernel Heap Allocation & Dynamic Data Structures ---"),
        ("Heap Baseline", "[Heap Baseline] Used:"),
        ("Step 1 Banner", "Demo Step 1: Allocating dynamic task buffer using Vec<DynamicTaskDescriptor>..."),
        ("Step 1 Result", "[Step 1 Result] Allocated 3 tasks in vector."),
        ("Step 2 Banner", "Demo Step 2: Allocating recursive tree node using Box<T>..."),
        ("Step 2 Result", "[Step 2 Result] Allocated 3-node binary tree. Root: 50, Left: 25, Right: 75."),
        ("Step 2 Metrics", "[Step 2 Metrics] Heap Used:"),
        ("Step 3 Banner", "Demo Step 3: Deallocating tree and task buffer, verifying free-list memory reclamation..."),
        ("Step 3 Result", "[Step 3 Result] Heap Used:"),
        ("Demo Success", "Demo Success: Kernel heap allocation, dynamic structures, and memory recycling verified."),
        ("Idle Loop Banner", "All Milestone 11 tasks and demonstrations completed. Entering idle loop."),
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

    # 9. Visual VGA Screendump Analysis
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
            print("  - Confirm that the memory test results and heap demonstration are rendered clearly on screen.")
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
