from PIL import Image
import numpy as np

img = Image.open("screenshots/vga_milestone16.png").convert("RGB")
arr = np.array(img)
flat = arr.reshape(-1, 3)
unique, counts = np.unique(flat, axis=0, return_counts=True)
print("=== Screenshot Color Analysis ===")
for u, c in zip(unique, counts):
    pct = c / len(flat) * 100.0
    hex_col = "#{:02X}{:02X}{:02X}".format(u[0], u[1], u[2])
    print(f"  {hex_col} RGB{tuple(u)}: {c:8} pixels ({pct:5.2f}%)")
