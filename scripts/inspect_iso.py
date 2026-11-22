#!/usr/bin/env python3
import struct

with open("iso/micro_os.iso", "rb") as f:
    data = f.read()

print("Total ISO size:", len(data), "bytes (", len(data)//2048, "CD sectors )")

for sec in range(len(data) // 2048):
    if data[sec*2048 : sec*2048+3] == b"\xeb\x3f\x90":
        print("cdboot.bin found at CD sector:", sec)
        bit = data[sec*2048+8 : sec*2048+64]
        pvd, boot_lba, file_len, csum = struct.unpack("<IIII", bit[:16])
        print(f"Boot Info Table: pvd={pvd}, boot_lba={boot_lba}, file_len={file_len}, csum={csum}")

        # Check what is in sector sec + 1:
        next_sec = data[(sec+1)*2048 : (sec+2)*2048]
        print(f"Sector {sec+1} (expected bootimage start):")
        print("First 16 bytes:", next_sec[:16].hex())
        print("Magic at 510..512:", hex(next_sec[510]), hex(next_sec[511]))
