import zipfile
import os
import sys

def main():
    zip_path = "submission/rust_micro_os_source.zip"
    if not os.path.exists(zip_path):
        print("[-] Error: ZIP archive does not exist at:", zip_path)
        sys.exit(1)

    with zipfile.ZipFile(zip_path, 'r') as zf:
        namelist = zf.namelist()
        print(f"[+] Total files in ZIP: {len(namelist)}")
        print(f"[+] Archive test result (bad file check): {zf.testzip()}")

        expected = [
            "Cargo.toml",
            "Cargo.lock",
            "README.md",
            "x86_64-micro_os.json",
            "src/main.rs",
            "src/lib.rs",
            "scripts/build_iso.py",
            "scripts/shim.s",
            "scripts/test_milestone15.py",
            "docs/FINAL_PROJECT_REPORT.pdf",
            "docs/FINAL_PROJECT_REPORT.md",
            "screenshots/vga_milestone15.png"
        ]

        normalized = [n.replace('\\', '/') for n in namelist]
        missing = []
        for exp in expected:
            if exp in normalized:
                print(f"  [PASS] {exp}")
            else:
                print(f"  [FAIL] Missing: {exp}")
                missing.append(exp)

        if missing:
            print(f"[-] Verification failed: {len(missing)} files missing.")
            sys.exit(1)
        else:
            print("[+] All essential source files verified present in ZIP!")

if __name__ == "__main__":
    main()
