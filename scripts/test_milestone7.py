#!/usr/bin/env python3
"""
Milestone 7 Ready Queue Verification Audit Script
=================================================
Automated test suite verifying:
  1. Boot into 64-bit Long Mode with VGA and Serial console operational.
  2. Preserved Milestone 6 task tests (TC1..=TC7).
  3. Programmatic execution and verification of 12 in-kernel Ready Queue tests:
     - RQ-TC1: Empty queue behavior
     - RQ-TC2: Enqueue single and multiple tasks
     - RQ-TC3: Strict FIFO order
     - RQ-TC4: Peek inspection without removal
     - RQ-TC5: Duplicate task insertion cleanly rejected
     - RQ-TC6: Full queue capacity (32 entries) & overflow rejection
     - RQ-TC7: Dequeue underflow safety (returns None)
     - RQ-TC8: Arbitrary task removal with compaction
     - RQ-TC9: Circular buffer index wraparound consistency
     - RQ-TC10: State filtering (only Ready accepted)
     - RQ-TC11: Re-enqueueing unblocked tasks
     - RQ-TC12: Interleaved operations count tracking & clear()
  4. Overall Ready Queue test suite confirmation banner.
  5. Multi-task Ready Queue demonstration in strict sequential order.
  6. Premature QEMU exit detection vs intentional termination.
  7. Visual VGA screendump verification and color palette analysis.
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
        "-monitor", "tcp:127.0.0.1:55557,server,nowait"
    ]

    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 7 VERIFICATION AUDIT            ")
    print("=================================================================")
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, console test, in-kernel test suites, and RQ demo
    time.sleep(4.5)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone7.ppm"
    png_path = "screenshots/vga_milestone7.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55557), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone7.ppm\n")
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

    # 2. Verification of the 12 In-Kernel Ready Queue Tests
    expected_rq_tests = [
        ("RQ-TC1: Empty queue behavior", "RQ-TC1: Empty queue behavior (len=0, is_empty=true, dequeue=None) verified"),
        ("RQ-TC2: Enqueue single and multiple tasks", "RQ-TC2: Enqueue single and multiple tasks (count: 3) verified"),
        ("RQ-TC3: Strict FIFO order", "RQ-TC3: Strict FIFO order (T10 -> T20 -> T30 -> None) verified"),
        ("RQ-TC4: Peek inspection without removal", "RQ-TC4: Peek inspection without removal verified"),
        ("RQ-TC5: Duplicate task rejection", "RQ-TC5: Duplicate task insertion cleanly rejected"),
        ("RQ-TC6: Full queue capacity & overflow", "RQ-TC6: Full queue capacity (32 entries) & overflow rejection verified"),
        ("RQ-TC7: Dequeue underflow safety", "RQ-TC7: Dequeue from empty queue safely returns None without panic"),
        ("RQ-TC8: Specific task removal & compaction", "RQ-TC8: Removal of specific task (T3) and compaction verified (Order: T1->T2->T4)"),
        ("RQ-TC9: Circular buffer wraparound", "RQ-TC9: Circular buffer index wraparound consistency verified (30 items traversed)"),
        ("RQ-TC10: State filtering (only Ready accepted)", "RQ-TC10: State filtering (Running/Blocked/Terminated rejected, Ready accepted) verified"),
        ("RQ-TC11: Lifecycle re-enqueueing of unblocked tasks", "RQ-TC11: Lifecycle re-enqueueing of unblocked tasks verified"),
        ("RQ-TC12: Interleaved operations count tracking", "RQ-TC12: Interleaved operations count tracking & clear() verified"),
    ]

    print("\n[TEST SUITE 1] In-Kernel Ready Queue Programmatic Test Suite Verification:")
    for name, tag in expected_rq_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"In-kernel test failed or missing: {name}")

    if ">>> MILESTONE 7 KERNEL TESTS: ALL 12 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Ready Queue Test Suite Confirmation: ALL 12 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Ready Queue Test Suite confirmation banner missing!")
        failures.append("Ready Queue test suite overall summary missing")

    # 3. Verification of the 13 In-Kernel Ready Queue Regression Tests (Auto-Sync & Validation)
    expected_rq_reg_tests = [
        ("RQ-REG-1: Running task rejected", "RQ-REG-1: Running task rejected by public enqueue API (TaskNotReady)"),
        ("RQ-REG-2: Blocked task rejected", "RQ-REG-2: Blocked task rejected by public enqueue API (TaskNotReady)"),
        ("RQ-REG-3: Terminated task rejected", "RQ-REG-3: Terminated task rejected by public enqueue API (TaskNotReady)"),
        ("RQ-REG-4: Nonexistent TaskId rejected", "RQ-REG-4: Nonexistent TaskId rejected (TaskNotFound)"),
        ("RQ-REG-5: Ready task validated & enqueued", "RQ-REG-5: Ready task successfully validated and enqueued"),
        ("RQ-REG-6: Ready -> Running auto-removes", "RQ-REG-6: Ready -> Running automatically removed task from Ready Queue"),
        ("RQ-REG-7: Lifecycle (Ready->Running->Blocked->Ready) auto-sync", "RQ-REG-7: Lifecycle (Ready->Running->Blocked->Ready) auto-sync verified; Ready->Blocked rejected"),
        ("RQ-REG-8: Ready -> Terminated auto-removes", "RQ-REG-8: Ready -> Terminated automatically removed task from Ready Queue"),
        ("RQ-REG-9: Blocked -> Ready appends tail", "RQ-REG-9: Blocked -> Ready automatically enqueued task at queue tail"),
        ("RQ-REG-10: Running -> Ready appends tail", "RQ-REG-10: Running -> Ready (preemption) automatically enqueued task at queue tail"),
        ("RQ-REG-11: Repeated Ready no duplicate", "RQ-REG-11: Repeated Ready transitions do not create duplicate queue entries"),
        ("RQ-REG-12: Failed transition preserves state & queue", "RQ-REG-12: Failed transition leaves both task state and Ready Queue unchanged"),
        ("RQ-REG-13: Queue-full transition fails safely", "RQ-REG-13: Queue-full transition failure safely preserves task state and queue contents"),
    ]

    print("\n[TEST SUITE 2] Ready Queue Auto-Sync & Validation Regression Test Suite Verification:")
    for name, tag in expected_rq_reg_tests:
        if tag in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] {name} - Missing expected output tag: '{tag}'")
            failures.append(f"Regression test failed or missing: {name}")

    if ">>> MILESTONE 7 REGRESSION TESTS: ALL 13 TESTS PASSED <<<" in stdout:
        print("  [PASS] Overall Ready Queue Regression Test Suite Confirmation: ALL 13 TESTS PASSED.")
    else:
        print("  [FAIL] Overall Ready Queue Regression Test Suite confirmation banner missing!")
        failures.append("Ready Queue regression test suite overall summary missing")

    # 4. Verification of Ready Queue Demonstration in Exact Order
    print("\n[TEST SUITE 3] Ready Queue Demonstration (Sequential Order Check):")
    demo_ordered_steps = [
        ("RQ-Demo Step 1: Create 4 Tasks", "RQ-Demo Step 1: Created tasks"),
        ("RQ-Demo Step 2: Enqueue in Order", "RQ-Demo Step 2: Enqueued tasks in order:"),
        ("RQ-Demo Step 3: Display Queue Count & Peek", "RQ-Demo Step 3: Ready Queue count: 4/32"),
        ("RQ-Demo Step 4: Dequeue First Task", "RQ-Demo Step 4: Dequeued head task:"),
        ("RQ-Demo Step 5: Dequeue Second Task (FIFO)", "RQ-Demo Step 5: Dequeued next task:"),
        ("RQ-Demo Step 6: Duplicate Insertion Rejected", "RQ-Demo Step 6: Duplicate insertion of"),
        ("RQ-Demo Step 7: Ready -> Running Auto-Removal", "RQ-Demo Step 7: Dispatched"),
        ("RQ-Demo Step 8: Running -> Blocked Transition", "RQ-Demo Step 8: Task"),
        ("RQ-Demo Step 9: Blocked -> Ready Auto-Append Tail", "RQ-Demo Step 9: Task"),
        ("RQ-Demo Step 10: Running -> Ready Auto-Append Tail", "RQ-Demo Step 10: Task"),
        ("RQ-Demo Step 11: Final Queue Verification", "RQ-Demo Step 11: Final queue verification:"),
        ("Idle Loop Message", "All Milestone 7 tasks and tests completed. Entering idle loop."),
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
            print("  - Please inspect `screenshots/vga_milestone7.png`.")
            print("  - Confirm that the Ready Queue demo and summary table are rendered clearly on screen.")
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
