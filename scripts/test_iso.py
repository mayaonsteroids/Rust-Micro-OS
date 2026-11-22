#!/usr/bin/env python3
import subprocess
import time
import socket
import os

qemu_cmd = [
    "qemu-system-x86_64",
    "-boot", "d",
    "-cdrom", "iso/micro_os.iso",
    "-serial", "stdio",
    "-display", "none",
    "-monitor", "tcp:127.0.0.1:55569,server,nowait"
]
print("Running QEMU...")
proc = subprocess.Popen(qemu_cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
time.sleep(2)

try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(("127.0.0.1", 55569))
    time.sleep(0.5)
    s.recv(1024)
    s.sendall(b"screendump screenshots/test_iso.ppm\n")
    time.sleep(0.5)
    s.close()
    from PIL import Image
    img = Image.open("screenshots/test_iso.ppm")
    img.save("screenshots/test_iso.png")
    print("Saved screenshots/test_iso.png")
except Exception as e:
    print("Monitor error:", e)

time.sleep(1)
proc.terminate()
stdout, stderr = proc.communicate()
print("STDOUT len:", len(stdout))
print("STDOUT preview:\n", stdout[:500])
print("STDERR preview:\n", stderr[:500])
