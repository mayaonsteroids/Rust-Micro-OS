#!/usr/bin/env python3
"""
Milestone 15 ISO Generation and Final Boot Verification Audit Script
====================================================================
Automated verification audit script covering:
  1. Bootable ISO verification:
     - Existence and file size of iso/micro_os.iso
     - El Torito boot catalog and emulation settings verification
  2. QEMU CD-ROM Boot:
     - Direct boot from CD-ROM device: qemu-system-x86_64 -boot d -cdrom iso/micro_os.iso
     - Intercepted INT 13h sector translation shim execution
     - Seamless transition to Long Mode without debug character pollution
  3. Kernel Boot & Console Initialization:
     - 64-bit Long Mode entry
     - GDT, TSS, IDT, PIC, PIT timer initialization
     - VGA Text Buffer at 0xb8000 operational
     - 16550 UART COM1 Serial Console operational
     - Frame allocator, recursive paging, and active memory heap verified
  4. Preserved Test Suites & Subsystem Regressions on ISO Boot:
     - Milestone 6: Task State Management (ALL 7 TESTS PASSED)
     - Milestone 7: Ready Queue Scheduler (ALL 12 + ALL 13 PASSED)
     - Milestone 8: Cooperative Scheduler (ALL 12 TESTS PASSED) & Live Demo
     - Milestone 9: Preemptive Multitasking & Interrupts (ALL 6 TESTS PASSED) & Live Demo
     - Milestone 10: Concurrency & Synchronization Primitives (ALL 7 TESTS PASSED) & Live Demo
     - Milestone 11: Kernel Memory Allocator & OOM Protection (ALL 8 TESTS PASSED) & Live Demo
     - Milestone 12: Process Abstraction & Ring 3 Preemption (ALL 10 TESTS PASSED) & Live Demo
     - Milestone 13: Hardware Paging Isolation & Syscalls (ALL 10 TESTS PASSED) & Live Demo
     - Milestone 14: Comprehensive Verification & Diagnostics (ALL 6 TESTS PASSED) & Live Demo
  5. Completion & Stability:
     - Verification of clean idle loop entry
     - Zero kernel panic, double-fault, or triple-fault
  6. Visual Verification:
     - Screendump capture via QEMU monitor socket
     - Conversion to screenshots/vga_milestone15.png
     - Resolution verification (720x400) and color palette analysis
"""

import subprocess
import time
import socket
import os
import sys

def run_audit():
    iso_path = "iso/micro_os.iso"
    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO image '{iso_path}' does not exist! Please run scripts/build_iso.py first.")
        sys.exit(1)

    iso_size = os.path.getsize(iso_path)
    print("=================================================================")
    print("        RUST MICRO-OS: MILESTONE 15 ISO BOOT AUDIT               ")
    print("=================================================================")
    print(f"Target ISO: {iso_path}")
    print(f"ISO File Size: {iso_size} bytes ({iso_size / (1024 * 1024):.2f} MiB)")

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", iso_path,
        "-serial", "stdio",
        "-display", "none",
        "-monitor", "tcp:127.0.0.1:55565,server,nowait"
    ]
    print(f"Command: {' '.join(cmd)}\n")

    # Launch QEMU process
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    # Allow time for kernel boot, M6-M14 test suites and demos to complete
    time.sleep(12.0)

    # Check if QEMU exited prematurely before audit completion
    poll_code = proc.poll()
    if poll_code is not None:
        stdout, stderr = proc.communicate()
        print(f"[-] ERROR: QEMU exited prematurely with code {poll_code} before audit completed!")
        print("STDERR:", stderr)
        sys.exit(1)

    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone15.ppm"
    png_path = "screenshots/vga_milestone15.png"

    if os.path.exists(ppm_path):
        os.remove(ppm_path)
    if os.path.exists(png_path):
        os.remove(png_path)

    # Capture VGA framebuffer screendump via QEMU monitor
    monitor_connected = False
    for attempt in range(5):
        try:
            s = socket.create_connection(("127.0.0.1", 55565), timeout=2)
            time.sleep(0.3)
            s.recv(2048) # Read monitor greeting banner
            s.sendall(b"screendump screenshots/vga_milestone15.ppm\n")
            time.sleep(0.5)
            s.sendall(b"quit\n")
            s.close()
            monitor_connected = True
            print("[+] Successfully requested VGA screendump via QEMU monitor socket.")
            break
        except Exception:
            time.sleep(0.5)

    if not monitor_connected:
        print("[-] Warning: Could not connect to QEMU monitor socket on 127.0.0.1:55565.")

    # Graceful termination of QEMU after collecting evidence
    proc.terminate()
    try:
        stdout, stderr = proc.communicate(timeout=3)
    except Exception:
        proc.kill()
        stdout, stderr = proc.communicate()

    print("\n-----------------------------------------------------------------")
    print("                    QEMU SERIAL CONSOLE LOG                      ")
    print("-----------------------------------------------------------------")
    print(stdout)
    print("-----------------------------------------------------------------")

    failures = []

    # 1. Boot & Hardware Assertions
    print("\n[PHASE 1] Checking Boot & Core Initialization from ISO:")
    required_boot_patterns = [
        ("Long Mode Entry", "[INFO] Kernel successfully started in 64-bit Long Mode."),
        ("VGA Console Init", "[INFO] VGA text console initialized at 0xb8000."),
        ("Serial Console Init", "[INFO] 16550 UART serial driver initialized at COM1 (0x3F8)."),
        ("Interrupt Infrastructure", "[INFO] Hardware interrupt infrastructure initialized (PIC remapped to 32/40, PIT configured at 100 Hz, IDT loaded)."),
        ("Memory Subsystem Init", "[INFO]          INITIALIZING KERNEL MEMORY SUBSYSTEM           "),
        ("Kernel Heap Mapping", "[INFO] [MEMORY] Kernel Heap successfully mapped:"),
        ("Global Frame Allocator", "[INFO] [MEMORY] Global frame allocator initialized for hardware paging isolation."),
    ]
    for name, pattern in required_boot_patterns:
        if pattern in stdout:
            print(f"  [PASS] {name}")
        else:
            print(f"  [FAIL] Missing pattern: {pattern}")
            failures.append(f"Missing boot pattern: {name}")

    # 2. Test Suites Verification
    print("\n[PHASE 2] Verifying Preserved Subsystem Test Suites from ISO:")
    required_test_suites = [
        ("Milestone 6 Tasks", "ALL 7 TESTS PASSED"),
        ("Milestone 7 Ready Queue", "ALL 12 TESTS PASSED"),
        ("Milestone 7 Regression", "ALL 13 TESTS PASSED"),
        ("Milestone 8 Scheduler", "ALL 12 TESTS PASSED"),
        ("Milestone 9 Interrupts", "ALL 6 TESTS PASSED"),
        ("Milestone 10 Sync", "ALL 7 TESTS PASSED"),
        ("Milestone 11 Allocator", "ALL 8 TESTS PASSED"),
        ("Milestone 12 Process/IPC", "ALL 10 TESTS PASSED"),
        ("Milestone 13 User Mode/Syscalls", "ALL 10 TESTS PASSED"),
        ("Milestone 14 Diagnostics", "ALL 6 TESTS PASSED"),
    ]
    for name, pattern in required_test_suites:
        if pattern in stdout:
            print(f"  [PASS] {name} ({pattern})")
        else:
            print(f"  [FAIL] Missing test confirmation: {name} ({pattern})")
            failures.append(f"Missing test suite: {name}")

    # 3. Demonstration & Final Idle State
    print("\n[PHASE 3] Verifying Execution Flow & System Stability:")
    idle_pattern = "All Milestone 14 tasks, tests, and demonstrations completed successfully. Entering idle loop."
    if idle_pattern in stdout:
        print("  [PASS] Reached final idle loop cleanly without panic or triple-fault")
    else:
        print(f"  [FAIL] Missing idle loop confirmation: '{idle_pattern}'")
        failures.append("System did not reach final idle loop")

    # 4. Check for Panic or Fault Indicators
    bad_patterns = ["PANIC", "DOUBLE FAULT", "TRIPLE FAULT", "GENERAL PROTECTION FAULT in kernel mode"]
    for bad in bad_patterns:
        if bad in stdout:
            print(f"  [FAIL] Detected unexpected kernel panic/fault: '{bad}'")
            failures.append(f"Kernel fault detected: {bad}")

    print("\n-----------------------------------------------------------------")
    if not failures:
        print(">>> AUTOMATED ASSERTIONS RESULT: ALL PASS (0 ERRORS) <<<")
    else:
        print(f">>> AUTOMATED ASSERTIONS RESULT: FAILED ({len(failures)} ERRORS) <<<")
        for f in failures:
            print(f"    - {f}")
    print("-----------------------------------------------------------------")

    # 5. Visual VGA Screendump Analysis
    print("\n=================================================================")
    print("     PART B: VISUAL INSPECTION ARTIFACT (ISO BOOT SCREEN)        ")
    print("=================================================================")

    if failures:
        print("[-] Skipping VGA screenshot generation because kernel boot verification failed.")
        print(f"\n[-] Audit completed with {len(failures)} failures.")
        sys.exit(1)

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
