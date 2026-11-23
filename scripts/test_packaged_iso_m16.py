#!/usr/bin/env python3
"""
Verification of Packaged ISO (submission/micro_os_milestone16.iso) in QEMU.
Boots the CD-ROM ISO image directly with El Torito emulation,
waits for the interactive shell prompt 'micro-os> ',
and runs interactive shell commands with live verification.
"""

import subprocess
import time
import os
import sys

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    iso_path = os.path.join(base_dir, "submission", "micro_os_milestone16.iso")

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: Packaged ISO not found at {iso_path}")
        sys.exit(1)

    print(f"[*] Packaged ISO: {iso_path} ({os.path.getsize(iso_path)} bytes)")

    cmd = [
        "qemu-system-x86_64",
        "-boot", "d",
        "-cdrom", iso_path,
        "-serial", "stdio",
        "-display", "none",
        "-m", "128M",
    ]

    print(f"[*] Launching QEMU: {' '.join(cmd)}")
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0,
    )

    full_output = ""

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
                sys.stdout.write(ch)
                sys.stdout.flush()
                if target in buf:
                    return buf
            else:
                time.sleep(0.005)
        return buf

    def send_cmd(cmd_str: str, wait_for_prompt: bool = True, timeout: float = 8.0) -> str:
        print(f"\n[SHELL-INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for_prompt:
            return read_until("micro-os> ", timeout=timeout)
        return ""

    try:
        print("[*] Waiting for optical boot and interactive shell prompt 'micro-os>' (timeout: 25s)...")
        initial = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in initial:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        print("\n[+] Successfully acquired interactive shell prompt on optical CD-ROM boot!")

        # Step 1: list
        out = send_cmd("list\n", wait_for_prompt=True, timeout=8.0)
        assert "calc" in out, "list command missing 'calc'"
        print("  [+] `list` command output verified!")

        # Step 2: run hello
        out = send_cmd("run hello\n", wait_for_prompt=True, timeout=8.0)
        assert "Hello from isolated Ring 3 user space!" in out, "hello failed"
        assert "completed with exit code 0" in out, "hello exit code non-zero"
        print("  [+] `run hello` verified (Ring 3 SYS_WRITE, SYS_EXIT 0)!")

        # Step 3: run counter
        out = send_cmd("run counter\n", wait_for_prompt=True, timeout=8.0)
        assert "Counter program finished (5/5)." in out, "counter failed"
        assert "completed with exit code 0" in out, "counter exit code non-zero"
        print("  [+] `run counter` verified (Ring 3 SYS_YIELD loop, SYS_EXIT 0)!")

        # Step 4: run calc
        out = send_cmd("run calc\n", wait_for_prompt=True, timeout=8.0)
        assert "Calc: 42 * 10 + 7 = 427" in out, "calc failed"
        assert "completed with exit code 0" in out, "calc exit code non-zero"
        print("  [+] `run calc` verified (Ring 3 arithmetic, SYS_EXIT 0)!")

        # Step 5: mem
        out = send_cmd("mem\n", wait_for_prompt=True, timeout=8.0)
        assert "Recycled in pool:" in out, "mem command failed"
        print("  [+] `mem` command verified (physical memory metrics)!")

        print("\n=================================================================")
        print("    PACKAGED ISO (micro_os_milestone16.iso) VERIFICATION SUCCESS ")
        print("=================================================================")
        print("[+] Cold booted from optical CD-ROM media via El Torito.")
        print("[+] Interactive shell responsive and executed user commands.")
        print("[+] Verified run hello, run counter, run calc, list, and mem.")

        proc.kill()
        sys.exit(0)

    except Exception as e:
        print(f"\n[-] Exception during test: {e}")
        proc.kill()
        sys.exit(1)

if __name__ == "__main__":
    main()
