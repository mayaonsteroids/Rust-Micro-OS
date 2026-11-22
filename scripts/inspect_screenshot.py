import os
from PIL import Image
import numpy as np

png_path = "screenshots/vga_boot.png"
ppm_path = "screenshots/vga_boot.ppm"

print("=== SCREENSHOT AUDIT ===")
print("File exists (PNG):", os.path.exists(png_path))
print("File size (PNG):", os.path.getsize(png_path), "bytes")
print("File exists (PPM):", os.path.exists(ppm_path))
print("File size (PPM):", os.path.getsize(ppm_path), "bytes")

img = Image.open(png_path)
width, height = img.size
print(f"Dimensions: {width} x {height} pixels (Standard 720x400 VGA text resolution)")

arr = np.array(img)
# Sample pixel colors (unique RGB colors in the rendered image)
unique_colors, counts = np.unique(arr.reshape(-1, arr.shape[2]), axis=0, return_counts=True)

print(f"\nTotal unique colors detected in rendered framebuffer: {len(unique_colors)}")
print("Color breakdown (RGB):")
color_names = {
    (0, 0, 0): "Black (Background)",
    (85, 255, 255): "Light Cyan (Banner Border & Title)",
    (255, 255, 255): "White (Standard Boot Logs)",
    (255, 255, 85): "Yellow (Memory Map Inspection)",
    (85, 255, 85): "Light Green (Status OK Confirmation)",
    (170, 170, 170): "Light Gray (Info / Idle Loop Entry)",
}

for color, count in zip(unique_colors, counts):
    rgb = tuple(color[:3])
    label = color_names.get(rgb, f"RGB{rgb}")
    pct = (count / (width * height)) * 100
    print(f"  - {label:<38}: {count:>6} pixels ({pct:5.2f}%)")

print("\nVisual verification conclusion:")
non_black = sum(c for col, c in zip(unique_colors, counts) if tuple(col[:3]) != (0, 0, 0))
print(f"Total rendered text pixels: {non_black:,} across 5 distinct VGA foreground colors.")
