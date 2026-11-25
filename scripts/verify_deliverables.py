import zipfile
import hashlib
import os

sub = '/mnt/e/MICRO OS/submission'

# 1. Test ZIPs
for zname in ['rust_micro_os_source.zip', 'Performance-Investigation.zip', 'CA5-Linux-Process-Investigator.zip', 'CA3-Process-vs-Thread.zip']:
    p = os.path.join(sub, zname)
    with zipfile.ZipFile(p, 'r') as zf:
        bad = zf.testzip()
        names = zf.namelist()
        sz = os.path.getsize(p)
        h = hashlib.sha256(open(p, 'rb').read()).hexdigest()
        print(f"=== {zname} ===")
        print(f"  Size: {sz} bytes")
        print(f"  SHA-256: {h}")
        print(f"  Corrupt test: {bad if bad else 'OK'}")
        print(f"  File count: {len(names)}")
        print(f"  Files: {names}\n")

# 2. Test PDF
pdf_path = os.path.join(sub, 'Operating-Systems-Lab-Record.pdf')
pdf_sz = os.path.getsize(pdf_path)
pdf_h = hashlib.sha256(open(pdf_path, 'rb').read()).hexdigest()
print("=== Operating-Systems-Lab-Record.pdf ===")
print(f"  Size: {pdf_sz} bytes")
print(f"  SHA-256: {pdf_h}")
