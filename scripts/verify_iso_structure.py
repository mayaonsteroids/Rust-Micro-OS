import struct
import os
import sys

def main():
    iso_path = "iso/micro_os.iso"
    bootimage_path = "target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin"

    with open(iso_path, "rb") as f:
        # LBA 34 = 34 * 2048 = 69632
        f.seek(34 * 2048)
        shim_data = f.read(2048)
        bootimage_in_iso = f.read()

    with open(bootimage_path, "rb") as f:
        orig_bootimage = f.read()

    print(f"Shim size in ISO: {len(shim_data)} bytes")
    pvd, file_lba, length, csum = struct.unpack("<IIII", shim_data[8:24])
    print(f"Boot Info Table: PVD={pvd}, File LBA={file_lba}, Length={length}, Checksum=0x{csum:08x}")
    print(f"File LBA check: expected 34, got {file_lba} -> {'MATCH' if file_lba == 34 else 'MISMATCH'}")
    
    # Check that bootimage inside cdboot.bin matches original bootimage exactly
    bootimage_len = len(orig_bootimage)
    actual_bootimage_part = bootimage_in_iso[:bootimage_len]
    print(f"Original bootimage size: {bootimage_len} bytes")
    print(f"Bootimage in ISO slice size: {len(actual_bootimage_part)} bytes")
    matches = (actual_bootimage_part == orig_bootimage)
    print(f"Bootimage in ISO exactly matches original binary: {matches}")
    
    sec0_magic = actual_bootimage_part[510:512]
    sec0_word = struct.unpack("<H", sec0_magic)[0]
    print(f"Sector 0 boot signature: 0x{sec0_word:04x} -> {'VALID (0xAA55)' if sec0_word == 0xAA55 else 'INVALID'}")

if __name__ == "__main__":
    main()
