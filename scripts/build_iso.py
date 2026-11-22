#!/usr/bin/env python3
import os
import subprocess
import sys

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    bootimage_path = os.path.join(base_dir, "target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin")
    shim_s = os.path.join(base_dir, "scripts/shim.s")
    shim_bin = os.path.join(base_dir, "scripts/shim.bin")
    staging_dir = os.path.join(base_dir, "iso/staging")
    iso_dir = os.path.join(base_dir, "iso")
    iso_output = os.path.join(iso_dir, "micro_os.iso")

    if not os.path.exists(bootimage_path):
        print(f"Error: Bootimage not found at {bootimage_path}")
        print("Please run `cargo bootimage --target x86_64-unknown-none` first.")
        sys.exit(1)

    print("[*] Assembling El Torito boot shim...")
    cmd_as = ["as", "--32", "-o", os.path.join(base_dir, "scripts/shim.o"), shim_s]
    subprocess.check_call(cmd_as)
    cmd_objcopy = ["objcopy", "-O", "binary", os.path.join(base_dir, "scripts/shim.o"), shim_bin]
    subprocess.check_call(cmd_objcopy)

    shim_size = os.path.getsize(shim_bin)
    if shim_size != 2048:
        print(f"Error: Shim binary must be exactly 2048 bytes (1 CD sector), got {shim_size} bytes")
        sys.exit(1)
    print(f"[+] Assembled shim binary: {shim_size} bytes")

    print("[*] Preparing ISO staging directory...")
    boot_staging = os.path.join(staging_dir, "boot")
    os.makedirs(boot_staging, exist_ok=True)

    # Combine shim + bootimage into cdboot.bin
    cdboot_path = os.path.join(boot_staging, "cdboot.bin")
    with open(cdboot_path, "wb") as out_f:
        with open(shim_bin, "rb") as in_shim:
            out_f.write(in_shim.read())
        with open(bootimage_path, "rb") as in_boot:
            out_f.write(in_boot.read())

    cdboot_size = os.path.getsize(cdboot_path)
    print(f"[+] Generated El Torito boot payload: {cdboot_path} ({cdboot_size} bytes)")

    # Copy informational files to ISO root
    subprocess.call(["cp", os.path.join(base_dir, "README.md"), staging_dir])
    if os.path.exists(os.path.join(base_dir, "LICENSE")):
        subprocess.call(["cp", os.path.join(base_dir, "LICENSE"), staging_dir])

    print("[*] Generating bootable ISO using xorriso...")
    cmd_xorriso = [
        "xorriso", "-as", "mkisofs",
        "-R", "-r", "-J",
        "-volid", "RUST_MICRO_OS",
        "-b", "boot/cdboot.bin",
        "-no-emul-boot",
        "-boot-load-size", "4",
        "-boot-info-table",
        "-o", iso_output,
        staging_dir
    ]

    print("Running:", " ".join(cmd_xorriso))
    subprocess.check_call(cmd_xorriso)

    iso_size = os.path.getsize(iso_output)
    print(f"[+] Successfully generated bootable ISO: {iso_output} ({iso_size} bytes / {iso_size / 1024 / 1024:.2f} MiB)")

if __name__ == "__main__":
    main()
