#!/usr/bin/env python3
with open("target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin", "rb") as f:
    data = f.read()

print("Total size:", len(data), "bytes")
print("Sector 0 magic:", hex(data[510]), hex(data[511]))
elf_pos = data.find(b"\x7fELF")
print("ELF magic at offset:", elf_pos, "sector:", elf_pos // 512 if elf_pos != -1 else "none")

# Check partition table in sector 0
pt = data[446:510]
print("Partition table entry 1:", pt[:16].hex())
