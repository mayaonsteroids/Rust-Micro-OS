#!/usr/bin/env python3
"""
Milestone 6 Task Representation and Task States Verification Audit Script
=========================================================================
Automated test suite verifying:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Programmatic execution and verification of 7 in-kernel task tests:
     - TC1: Task creation and initial READY state
     - TC2: Unique sequential Task ID assignment
     - TC3: Valid life-cycle state transitions
     - TC4: Invalid state transition rejection
     - TC5: Task termination and terminal state permanence
     - TC6: TaskManager registration, lookup, and active counting
     - TC7: TaskManager capacity limits (32 tasks)
  3. Overall kernel test suite confirmation message.
  4. Multi-task lifecycle demonstration messages in strict sequential order.
  5. Distinction between intentional process termination and premature QEMU failure.
  6. Visual VGA screendump verification with accurate pixel color reporting.
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
        "-monitor", "tcp:127.0.0.1:55556,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 6 VERIFICATION AUDIT            ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, in-kernel test suite, and task demo
    time.sleep(3.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone6.ppm"
    png_path = "screenshots/vga_milestone6.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55556), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone6.ppm\n")
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

    # 1. Verification of the 7 In-Kernel Task Tests
    expected_tests = [
        ("TC1: Task creation & initial READY state", "TC1: Task creation & initial READY state verified"),
        ("TC2: Unique sequential Task IDs", "TC2: Unique sequential Task IDs verified"),
        ("TC3: Valid state transitions", "TC3: Valid state transitions (Ready->Running->Blocked->Ready->Terminated) verified"),
        ("TC4: Invalid transition rejection", "TC4: Illegal transitions (Terminated->*, Blocked->Running, Ready->Blocked) correctly rejected"),
        ("TC5: Task termination & finality", "TC5: Task termination and terminal state permanence verified"),
        ("TC6: TaskManager lookup & tracking", "TC6: TaskManager registration, lookup, and active counting verified"),
        ("TC7: TaskManager capacity limits", "TC7: TaskManager capacity limit (32 tasks) safely enforced"),
    ]

    print("[TEST SUITE 1] In-Kernel Programmatic Test Suite Verification:")
    for name, tag in expected_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel test failed or missing: {name}")

    if ">>> MILESTONE 6 KERNEL TESTS: ALL 7 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Kernel Test Suite Confirmation: ALL 7 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Kernel Test Suite confirmation message missing!")
        failures.append("Kernel test suite overall summary missing")

    # 2. Verification of Multi-Task Lifecycle Demonstration in Exact Order
    print("\n[TEST SUITE 2] Multi-Task Lifecycle Demonstration (Sequential Order Check):")
    demo_ordered_steps = [
        ("Task Creation", "Created 3 tasks:"),
        ("Dispatch ComputationTask to RUNNING", "state is now RUNNING."),
        ("Complete ComputationTask to TERMINATED", "state is now TERMINATED."),
        ("Dispatch IOHandlerTask to RUNNING", "state is now RUNNING."),
        ("Block IOHandlerTask on I/O Event", "BLOCKED: waiting on simulated I/O event"),
        ("Unblock IOHandlerTask to READY", "unblocked, state is now READY."),
        ("Active Task Table Header", "--- Active Task Table Summary ---"),
        ("Active Task Table Counts", "Total Registered: 3 | Active: 2"),
        ("Active Task Table IdleTask Entry", "Name: IdleTask         State: READY"),
        ("Active Task Table ComputationTask Entry", "Name: ComputationTask  State: TERMINATED"),
        ("Active Task Table IOHandlerTask Entry", "Name: IOHandlerTask    State: READY"),
        ("Idle Loop Message", "All Milestone 6 tasks and tests completed. Entering idle loop."),
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
            print("  - Please inspect `screenshots/vga_milestone6.png`.")
            print("  - Confirm that the Task Management demo and summary table are rendered clearly on screen.")
            print("  - Confirm distinct color codes for [INFO], [WARN], and [DEBUG] log tags.")
        except Exception as e:
            print(f"[-] ERROR: Failed to open or inspect screenshot: {e}")
            failures.append(f"Screenshot open/inspect failed: {e}")
    else:
        print("[-] Screenshot PPM file not found or empty.")
        failures.append("Screenshot PPM missing")

    # Final execution status: distinguishes intentional termination from assertion failure
    print(f"\n[EXECUTION STATUS] QEMU was intentionally terminated after test completion: {proc.returncode is not None}")
    if failures:
        print(f"[-] Audit completed with {len(failures)} failures.")
        sys.exit(1)
    else:
        print("[+] Audit completed successfully with 0 errors.")
        sys.exit(0)

if __name__ == "__main__":
    run_audit()
