#!/usr/bin/env bash
set -e
export PATH="$HOME/.cargo/bin:$PATH"

TEMP_DIR="/tmp/micro_os_m16_test_extract"
echo "[*] Cleaning and creating temporary test directory: $TEMP_DIR"
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

echo "[*] Extracting submission/rust_micro_os_milestone16_source.zip..."
python3 -m zipfile -e "submission/rust_micro_os_milestone16_source.zip" "$TEMP_DIR"
cd "$TEMP_DIR"
echo "[+] Extracted $(find . -type f | wc -l) files successfully."

echo "[*] Verifying no git, caches, or build artifacts were included..."
if [ -d "target" ]; then echo "[-] ERROR: target/ directory found in ZIP!"; exit 1; fi
if [ -d ".git" ]; then echo "[-] ERROR: .git/ directory found in ZIP!"; exit 1; fi
if [ -f "serial.log" ]; then echo "[-] ERROR: serial.log found in ZIP!"; exit 1; fi

echo "[*] Compiling kernel with cargo check..."
cargo check

echo "[*] Building raw bootable image with cargo bootimage..."
cargo bootimage

BOOTIMAGE="target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin"
if [ ! -f "$BOOTIMAGE" ]; then
    echo "[-] ERROR: $BOOTIMAGE not found!"
    exit 1
fi
echo "[+] Successfully built raw bootimage: $(ls -lh "$BOOTIMAGE" | awk '{print $5, $9}')"

echo "[*] Mastering ISO with scripts/build_iso.py..."
python3 scripts/build_iso.py

ISO="iso/micro_os.iso"
if [ ! -f "$ISO" ]; then
    echo "[-] ERROR: $ISO not found!"
    exit 1
fi
echo "[+] Successfully built optical ISO: $(ls -lh "$ISO" | awk '{print $5, $9}')"

echo "[*] Running Milestone 16 verification in isolated directory..."
python3 scripts/test_milestone16.py

echo "[+] ISOLATED MILESTONE 16 REPRODUCIBILITY AUDIT: SUCCESSFUL!"
