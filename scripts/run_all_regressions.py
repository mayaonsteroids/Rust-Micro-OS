#!/usr/bin/env python3
"""
Full Regression Test Runner for Milestones 11–16
Executes each regression harness, verifies exit code 0, and records execution metrics.
"""

import subprocess
import time
import os
import sys

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    
    test_suites = [
        ("Milestone 16 (Shell & Ring 3)", "scripts/test_milestone16.py"),
        ("Milestone 16 (Packaged ISO Boot)", "scripts/test_packaged_iso_m16.py"),
        ("Milestone 15 (Optical ISO CD-ROM)", "scripts/test_milestone15.py"),
        ("Milestone 14 (Concurrency & Telemetry)", "scripts/test_milestone14.py"),
        ("Milestone 13 (Hardware Paging & Ring 3)", "scripts/test_milestone13.py"),
        ("Milestone 12 (Process & IPC)", "scripts/test_milestone12.py"),
        ("Milestone 11 (Frame Allocator & Heap)", "scripts/test_milestone11.py"),
    ]

    print("=================================================================")
    print("      RUST MICRO-OS: FULL REGRESSION AUDIT (MILESTONES 11–16)     ")
    print("=================================================================")
    
    results = []
    all_passed = True

    for name, script_rel in test_suites:
        script_path = os.path.join(base_dir, script_rel)
        print(f"\n[*] Running {name}: python3 {script_rel}...")
        start_t = time.time()
        
        res = subprocess.run(
            ["python3", script_path],
            cwd=base_dir,
            capture_output=True,
            text=True
        )
        elapsed = time.time() - start_t
        exit_code = res.returncode
        passed = (exit_code == 0)
        
        status_str = "PASS" if passed else f"FAIL (exit {exit_code})"
        print(f"    [{status_str}] in {elapsed:.2f}s (Exit Code: {exit_code})")
        if not passed:
            all_passed = False
            print("    --- STDERR / OUTPUT SAMPLE ---")
            print(res.stdout[-500:] if res.stdout else "")
            print(res.stderr[-500:] if res.stderr else "")
            
        results.append((name, script_rel, exit_code, elapsed, passed))

    print("\n" + "=" * 65)
    print("                 REGRESSION AUDIT CONSOLIDATED SUMMARY           ")
    print("=" * 65)
    print(f"{'SUBSYSTEM / TEST HARNESS':<38} {'EXIT':<6} {'TIME':<8} {'STATUS'}")
    print("-" * 65)
    for name, script_rel, code, elapsed, passed in results:
        status_txt = "PASS" if passed else "FAIL"
        print(f"{name:<38} {code:<6} {elapsed:.2f}s   {status_txt}")
    print("=" * 65)

    if all_passed:
        print("[+] ALL REGRESSION TEST SUITES PASSED WITH ZERO ERRORS (Exit 0)!")
        sys.exit(0)
    else:
        print("[-] ONE OR MORE TEST SUITES FAILED!")
        sys.exit(1)

if __name__ == "__main__":
    main()
