import subprocess
import time
import socket
import os
import sys

cmd = [
    "qemu-system-x86_64",
    "-drive", "format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin",
    "-serial", "stdio",
    "-display", "none",
    "-monitor", "tcp:127.0.0.1:55555,server,nowait"
]

print("=== 1. LAUNCHING QEMU FOR MILESTONE 4 TEST ===")
print("Command:", " ".join(cmd))
proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

# Wait 2 seconds for boot and VGA render
time.sleep(2.0)

# Connect to QEMU monitor to capture VGA screen dump
os.makedirs("screenshots", exist_ok=True)
ppm_path = "screenshots/vga_boot.ppm"
if os.path.exists(ppm_path):
    os.remove(ppm_path)

screendump_ok = False
try:
    s = socket.create_connection(("127.0.0.1", 55555), timeout=3)
    time.sleep(0.3)
    s.recv(2048) # Read greeting
    s.sendall(b"screendump screenshots/vga_boot.ppm\n")
    time.sleep(0.5)
    s.sendall(b"quit\n")
    s.close()
    screendump_ok = True
    print("[+] Successfully requested VGA screendump via QEMU monitor.")
except Exception as e:
    print(f"[-] Monitor socket communication note: {e}")

proc.terminate()
try:
    stdout, stderr = proc.communicate(timeout=3)
except Exception:
    proc.kill()
    stdout, stderr = proc.communicate()

print("\n=== 2. CAPTURED SERIAL OUTPUT (DUAL-OUTPUT VERIFICATION) ===")
print(stdout)

print("\n=== 3. VGA BUFFER SCREENSHOT VERIFICATION ===")
if os.path.exists(ppm_path) and os.path.getsize(ppm_path) > 0:
    size = os.path.getsize(ppm_path)
    print(f"[+] VGA screen dump captured successfully: {ppm_path} ({size:,} bytes)")
    
    # Convert PPM to PNG for easy viewing and analyze pixels
    try:
        from PIL import Image
        img = Image.open(ppm_path)
        png_path = "screenshots/vga_boot.png"
        img.save(png_path)
        print(f"[+] Converted VGA screenshot to standard PNG: {png_path} ({img.width}x{img.height})")
    except Exception as img_err:
        print(f"[*] Note on PNG conversion: {img_err}")
else:
    print(f"[-] Screendump file not created or empty.")

print("\n=== 4. QEMU EXIT STATUS ===")
print(f"Process returncode: {proc.returncode}")
