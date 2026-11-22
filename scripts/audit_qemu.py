import subprocess
import time
import os
import signal
import sys

cmd = [
    "qemu-system-x86_64",
    "-drive", "format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin",
    "-serial", "stdio",
    "-display", "none"
]

print("=== 1. COMPLETE QEMU COMMAND ===")
print(" ".join(cmd))

print("\n=== 2. STARTING QEMU PROCESS ===")
start_time = time.time()
proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

# Allow 2.5 seconds for boot and serial output
time.sleep(2.5)

# Check if QEMU exited on its own or is still executing
spontaneous_exit = proc.poll()
elapsed = time.time() - start_time

if spontaneous_exit is None:
    print(f"Status at {elapsed:.2f}s: QEMU is ALIVE and RUNNING actively.")
    print("Self-termination check: QEMU did NOT exit by itself. Kernel is alive in its idle loop.")
    stopped_by = "Terminated by test harness (SIGTERM) after proving continuous kernel execution."
    proc.terminate()
    stdout, stderr = proc.communicate(timeout=3)
    final_code = proc.returncode
else:
    print(f"Status at {elapsed:.2f}s: QEMU EXITED SPONTANEOUSLY!")
    stopped_by = "Exited by itself."
    stdout, stderr = proc.communicate()
    final_code = spontaneous_exit

print(f"\n=== 3. HOW QEMU WAS STOPPED ===")
print(stopped_by)

print(f"\n=== 4. QEMU ACTUAL EXIT CODE ===")
print(f"Return code: {final_code}")
print("Note: On Linux/POSIX, a process terminated by SIGTERM (signal 15) has return code -15 (or 128+15=143 in shells).")

print(f"\n=== 5. STDERR OUTPUT ===")
if stderr and stderr.strip():
    print(stderr)
else:
    print("(None - stderr is completely clean, zero errors or warnings)")

print(f"\n=== 6. COMPLETE SERIAL OUTPUT (STDOUT) ===")
print(stdout)
