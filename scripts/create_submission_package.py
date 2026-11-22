import os
import zipfile
import shutil
import sys

def create_package():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    submission_dir = os.path.join(base_dir, "submission")
    os.makedirs(submission_dir, exist_ok=True)

    zip_filename = "rust_micro_os_source.zip"
    zip_path = os.path.join(submission_dir, zip_filename)

    # Folders and files to include
    include_files = [
        "Cargo.toml",
        "Cargo.lock",
        "x86_64-micro_os.json",
        ".gitignore",
        "LICENSE",
        "README.md",
    ]

    include_dirs = [
        ".cargo",
        "src",
        "scripts",
        "docs",
        "screenshots",
        "results",
        "tests",
        "traces",
    ]

    # Exclude patterns
    exclude_extensions = {".ppm", ".o", ".tmp", ".pyc", ".swp", ".bk"}
    exclude_dirs = {"target", "staging", "__pycache__", ".git", "submission"}

    print("[*] Creating clean source archive...")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        # Add root files
        for rel_file in include_files:
            abs_path = os.path.join(base_dir, rel_file)
            if os.path.exists(abs_path):
                zf.write(abs_path, arcname=rel_file)
                print(f"  + Added file: {rel_file}")

        # Add directories
        for rel_dir in include_dirs:
            abs_dir = os.path.join(base_dir, rel_dir)
            if not os.path.exists(abs_dir):
                continue
            for root, dirs, files in os.walk(abs_dir):
                # Filter out excluded directories in-place
                dirs[:] = [d for d in dirs if d not in exclude_dirs]
                for file in files:
                    ext = os.path.splitext(file)[1].lower()
                    if ext in exclude_extensions:
                        continue
                    if file == "serial.log":
                        continue
                    abs_file = os.path.join(root, file)
                    rel_arc = os.path.relpath(abs_file, base_dir)
                    zf.write(abs_file, arcname=rel_arc)
                    print(f"  + Added file: {rel_arc}")

    zip_size = os.path.getsize(zip_path)
    print(f"[+] Successfully created source ZIP: {zip_path} ({zip_size} bytes / {zip_size/1024:.1f} KiB)")

    # Copy final deliverables into submission directory
    print("[*] Staging final deliverables into submission/ directory...")

    # 1. micro_os.iso
    iso_src = os.path.join(base_dir, "iso/micro_os.iso")
    iso_dst = os.path.join(submission_dir, "micro_os.iso")
    if os.path.exists(iso_src):
        shutil.copy2(iso_src, iso_dst)
        print(f"  + Copied ISO: {iso_dst} ({os.path.getsize(iso_dst)} bytes)")

    # 2. Raw bootimage
    bootimage_src = os.path.join(base_dir, "target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin")
    bootimage_dst = os.path.join(submission_dir, "bootimage-rust-micro-os.bin")
    if os.path.exists(bootimage_src):
        shutil.copy2(bootimage_src, bootimage_dst)
        print(f"  + Copied Raw Bootimage: {bootimage_dst} ({os.path.getsize(bootimage_dst)} bytes)")

    # 3. Final Report PDF
    pdf_src = os.path.join(base_dir, "docs/FINAL_PROJECT_REPORT.pdf")
    pdf_dst = os.path.join(submission_dir, "FINAL_PROJECT_REPORT.pdf")
    if os.path.exists(pdf_src):
        shutil.copy2(pdf_src, pdf_dst)
        print(f"  + Copied PDF Report: {pdf_dst} ({os.path.getsize(pdf_dst)} bytes)")

    # 4. Fresh QEMU screenshot
    vga_src = os.path.join(base_dir, "screenshots/vga_milestone15.png")
    vga_dst = os.path.join(submission_dir, "vga_milestone15.png")
    if os.path.exists(vga_src):
        shutil.copy2(vga_src, vga_dst)
        print(f"  + Copied VGA Screenshot: {vga_dst} ({os.path.getsize(vga_dst)} bytes)")

    print("\n[+] All deliverables staged successfully in:", submission_dir)

if __name__ == "__main__":
    create_package()
