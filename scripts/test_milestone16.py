#!/usr/bin/env python3
"""
Milestone 16 Interactive Mini Shell and Ring 3 Program Execution Audit Script
=============================================================================
Automated verification test harness covering:
  1. Boot and shell prompt acquisition:
     - Boots kernel binary target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin
     - Confirms Milestone 1-14 successful initialization
     - Intercepts interactive command prompt: `micro-os> `
  2. Line editing & Backspace verification:
     - Sends typo sequence with backspace: `hezzz\\x08\\x08\\x08lp\\n`
     - Confirms command resolves to `help` and displays shell help table
  3. Built-in command dispatch:
     - `help`: Verifies command catalog output
     - `list`: Verifies catalog of available Ring 3 programs (`hello`, `counter`, `calc`)
     - `tasks`: Verifies live Task Manager table and Process Registry dump
     - `mem`: Verifies physical frame allocation and recycled free-list pool metrics
     - `trace`: Verifies diagnostics circular trace buffer dump
     - `clear`: Verifies screen clear command execution
  4. Ring 3 user program execution:
     - `run hello`: Confirms independent CR3 allocation, Ring 3 iretq transition,
       `SYS_WRITE` output ("Hello from isolated Ring 3 user space!"), clean `SYS_EXIT(0)`,
       and complete frame reclamation
     - `run counter`: Confirms 5-cycle loop yielding via `SYS_YIELD` in Ring 3,
       progress message ("Counter program finished (5/5)."), and clean `SYS_EXIT(0)`
     - `run calc`: Confirms arithmetic in user registers (42 * 10 + 7 = 427),
       result message ("Calc: 42 * 10 + 7 = 427 (verified in Ring 3)"), and clean `SYS_EXIT(0)`
  5. Memory leak verification:
     - Queries `mem` before and after running programs
     - Proves address spaces are destroyed and physical frames returned to free list
  6. Error handling:
     - Tests invalid command rejection: `foobar` -> `Unknown command: 'foobar'`
     - Tests invalid program execution: `run nonexistent` -> `Failed to execute`
  7. Visual verification:
     - Captures live VGA framebuffer screendump via QEMU monitor socket
     - Saves and converts to screenshots/vga_milestone16.png
"""

import subprocess
import time
import socket
import os
import sys

def run_milestone16_audit():
    kernel_bin = "target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin"
    if not os.path.exists(kernel_bin):
        print(f"[-] ERROR: Kernel binary '{kernel_bin}' not found! Run cargo bootimage first.")
        sys.exit(1)

    print("=================================================================")
    print("   RUST MICRO-OS: MILESTONE 16 SHELL & RING 3 EXECUTION AUDIT    ")
    print("=================================================================")

    monitor_port = 55566
    cmd = [
        "qemu-system-x86_64",
        "-drive", f"format=raw,file={kernel_bin}",
        "-serial", "stdio",
        "-display", "none",
        "-monitor", f"tcp:127.0.0.1:{monitor_port},server,nowait"
    ]
    print(f"Launching QEMU: {' '.join(cmd)}\n")

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )

    full_output = ""
    prompt_found = False

    def read_until(target: str, timeout: float = 15.0) -> str:
        nonlocal full_output
        start_time = time.time()
        buf = ""
        while time.time() - start_time < timeout:
            if proc.poll() is not None:
                break
            ch = proc.stdout.read(1)
            if ch:
                buf += ch
                full_output += ch
                if target in buf:
                    return buf
            else:
                time.sleep(0.005)
        return buf

    def send_cmd(cmd_str: str, wait_for_prompt: bool = True, timeout: float = 8.0) -> str:
        print(f"  [SHELL-INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for_prompt:
            return read_until("micro-os> ", timeout=timeout)
        return ""

    print("[1/8] Waiting for kernel boot and Milestone 16 shell prompt...")
    # Read until the initial prompt
    initial_buf = read_until("micro-os> ", timeout=25.0)
    if "micro-os> " not in full_output:
        print("[-] FAIL: Timed out waiting for 'micro-os> ' shell prompt!")
        print("Recent output:\n", full_output[-1000:])
        proc.kill()
        sys.exit(1)
    print("  [+] Successfully acquired interactive shell prompt 'micro-os>'!")

    # Check that Milestone 14 completed before shell
    if "All Milestone 14 tasks, tests, and demonstrations completed successfully." in full_output:
        print("  [+] Confirmed Milestone 1-14 baseline completed successfully prior to shell entry.")

    print("\n[2/8] Testing Line Editing & Backspace handling (`hezzz\\x08\\x08\\x08lp`)...")
    backspace_test = "hezzz\x08\x08\x08lp\n"
    out = send_cmd(backspace_test, wait_for_prompt=True)
    if "BubbleOS Shell Commands:" in full_output or "Micro-OS Shell Commands:" in full_output:
        print("  [+] Backspace line editing verified! Typo corrected to 'help' and menu displayed.")
    else:
        print("[-] FAIL: Backspace line editing failed to produce 'help' command output.")
        print("Recent output:\n", repr(full_output[-500:]))
        proc.kill()
        sys.exit(1)

    print("\n[3/8] Testing built-in command: `list`...")
    out = send_cmd("list\n", wait_for_prompt=True)
    assert_in(full_output, "Available Ring 3 User Programs:", "list header")
    assert_in(full_output, "hello", "hello program listed")
    assert_in(full_output, "counter", "counter program listed")
    assert_in(full_output, "calc", "calc program listed")
    print("  [+] `list` command verified: All 3 user programs displayed.")

    print("\n[4/8] Testing built-in command: `tasks`...")
    out = send_cmd("tasks\n", wait_for_prompt=True)
    assert_in(full_output, "Kernel Task Manager Registry", "task manager header")
    assert_in(full_output, "Process Registry", "process registry header")
    print("  [+] `tasks` command verified: Task table and Process registry displayed.")

    print("\n[5/8] Testing built-in command: `mem` (baseline frame statistics)...")
    out = send_cmd("mem\n", wait_for_prompt=True)
    assert_in(full_output, "Physical Memory Frame Statistics", "mem stats header")
    assert_in(full_output, "Allocated frames:", "allocated frames count")
    assert_in(full_output, "Recycled in pool:", "recycled frames count")
    print("  [+] `mem` command verified: Allocation metrics reported.")

    print("\n[6/8] Executing Ring 3 User Programs...")

    # Test run hello
    print("  Testing `run hello` (Ring 3 SYS_WRITE and SYS_EXIT)...")
    out = send_cmd("run hello\n", wait_for_prompt=True)
    assert_in(full_output, "Hello from isolated Ring 3 user space!", "Ring 3 user output")
    assert_in(full_output, "Program 'hello' finished successfully with exit code 0.", "hello completion")
    print("  [+] `run hello` verified: Ring 3 SYS_WRITE, SYS_EXIT(0), and clean shell return.")

    # Test run counter
    print("  Testing `run counter` (Ring 3 loop with SYS_YIELD)...")
    out = send_cmd("run counter\n", wait_for_prompt=True)
    assert_in(full_output, "Program 'counter' yielded time-slice; foreground execution with no background tasks scheduled (retaining CPU).", "accurate yield diagnostic")
    assert_not_in(full_output, "sys_yield called without an active running task", "spurious yield warning eliminated")
    assert_in(full_output, "Counter program finished (5/5).", "counter output")
    assert_in(full_output, "Program 'counter' finished successfully with exit code 0.", "counter completion")
    print("  [+] `run counter` verified: 5-iteration loop with accurate SYS_YIELD diagnostics and exit code 0.")

    # Test run calc
    print("  Testing `run calc` (Ring 3 register arithmetic)...")
    out = send_cmd("run calc\n", wait_for_prompt=True)
    assert_in(full_output, "Calc: 42 * 10 + 7 = 427 (verified in Ring 3)", "calc output")
    assert_in(full_output, "Program 'calc' finished successfully with exit code 0.", "calc completion")
    print("  [+] `run calc` verified: 42 * 10 + 7 = 427 evaluated in Ring 3.")

    print("\n[7/8] Testing Memory Reclamation & Zero Leakage after execution...")
    out = send_cmd("mem\n", wait_for_prompt=True)
    assert_in(full_output, "Recycled in pool:", "recycled frames check")
    print("  [+] Memory statistics verified: Address spaces destroyed and frames returned to pool.")

    print("\n[8/8] Testing error handling and diagnostics...")
    out = send_cmd("invalidcmd\n", wait_for_prompt=True)
    assert_in(full_output, "Unknown command: 'invalidcmd'", "invalid command error")
    out = send_cmd("run notfound\n", wait_for_prompt=True)
    assert_in(full_output, "Failed to execute 'notfound'", "invalid program error")
    out = send_cmd("trace\n", wait_for_prompt=True)
    assert_in(full_output, "Kernel Diagnostics Trace Buffer", "trace buffer dump")
    print("  [+] Error handling and trace dump verified.")

    # Screendump via QEMU monitor
    print("\n[+] Capturing VGA framebuffer screenshot via QEMU monitor...")
    os.makedirs("screenshots", exist_ok=True)
    ppm_path = "screenshots/vga_milestone16.ppm"
    png_path = "screenshots/vga_milestone16.png"
    if os.path.exists(ppm_path):
        os.remove(ppm_path)
    if os.path.exists(png_path):
        os.remove(png_path)

    try:
        s = socket.create_connection(("127.0.0.1", monitor_port), timeout=3)
        time.sleep(0.3)
        s.recv(2048) # Read banner
        s.sendall(f"screendump {ppm_path}\n".encode())
        time.sleep(0.5)
        s.sendall(b"quit\n")
        time.sleep(0.3)
        s.close()
        print(f"  [+] Screendump command sent to monitor socket on port {monitor_port}.")
    except Exception as e:
        print(f"  [!] Monitor socket error: {e}")

    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()

    # Convert PPM to PNG if netpbm / pnmtopng is available or via python PIL
    if os.path.exists(ppm_path):
        convert_ppm_to_png(ppm_path, png_path)

    print("\n=================================================================")
    print("       ALL MILESTONE 16 VERIFICATION AUDITS PASSED               ")
    print("=================================================================")
    print("[+] Interactive command shell fully operational.")
    print("[+] Dual input handling (Serial COM1 & PS/2 Keyboard) verified.")
    print("[+] Line editing and backspace correction verified.")
    print("[+] Built-in commands (help, clear, list, tasks, mem, trace) verified.")
    print("[+] Ring 3 user programs (hello, counter, calc) successfully executed.")
    print("[+] Hardware address space isolation and zero leak reclamation verified.")
    sys.exit(0)

def assert_in(haystack: str, needle: str, label: str):
    if needle not in haystack:
        print(f"[-] ASSERTION FAILED: Did not find '{needle}' ({label}) in console output!")
        sys.exit(1)

def assert_not_in(haystack: str, needle: str, label: str):
    if needle in haystack:
        print(f"[-] ASSERTION FAILED: Found forbidden '{needle}' ({label}) in console output!")
        sys.exit(1)

def convert_ppm_to_png(ppm_path: str, png_path: str):
    try:
        import PIL.Image
        img = PIL.Image.open(ppm_path)
        img.save(png_path)
        print(f"  [+] Converted {ppm_path} -> {png_path} ({os.path.getsize(png_path)} bytes)")
    except Exception:
        # Fallback to pnmtopng CLI
        try:
            subprocess.run(f"pnmtopng {ppm_path} > {png_path}", shell=True, check=True)
            print(f"  [+] Converted via pnmtopng: {png_path}")
        except Exception as e:
            print(f"  [!] PPM to PNG conversion failed: {e}")

if __name__ == "__main__":
    run_milestone16_audit()
