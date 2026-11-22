#!/usr/bin/env bash
# Script to launch the Micro-OS in QEMU
set -e

BOOTIMAGE_PATH="target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin"

if [ ! -f "$BOOTIMAGE_PATH" ]; then
    echo "[!] Bootimage not found. Compiling with cargo bootimage..."
    cargo bootimage
fi

echo "[*] Launching Micro-OS in QEMU..."
qemu-system-x86_64 -drive format=raw,file="$BOOTIMAGE_PATH" -serial stdio
