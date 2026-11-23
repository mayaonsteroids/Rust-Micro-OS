#!/usr/bin/env python3
"""
Offline Audio Conversion Script for Rust Micro-OS AC97 Music Player.
Converts `assets/music/mayiiiii.mp3` into a compact 24 kHz Stereo IMA ADPCM
binary payload (`src/audio/mayiiiii_adpcm.bin`) optimized for continuous
hardware DMA streaming on Intel ICH AC97 (48 kHz 16-bit stereo PCM).
"""

import os
import sys
import numpy as np
import scipy.signal
import miniaudio

STEP_TABLE = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17,
    19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
    50, 55, 60, 66, 73, 80, 88, 97, 107, 118,
    130, 143, 157, 173, 190, 209, 230, 253, 279, 307,
    337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358,
    5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899,
    15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767
]
INDEX_TABLE = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8]

def encode_channel_adpcm(samples):
    predicted = 0
    step_idx = 0
    nibbles = np.zeros(len(samples), dtype=np.uint8)
    for i, sample in enumerate(samples):
        step = STEP_TABLE[step_idx]
        diff = int(sample) - predicted
        nibble = 0
        if diff < 0:
            nibble = 8
            diff = -diff
        mask = 4
        temp_step = step
        diffq = step >> 3
        for _ in range(3):
            if diff >= temp_step:
                nibble |= mask
                diff -= temp_step
                diffq += temp_step
            temp_step >>= 1
            mask >>= 1
        if nibble & 8:
            predicted = max(-32768, predicted - diffq)
        else:
            predicted = min(32767, predicted + diffq)
        step_idx = max(0, min(88, step_idx + INDEX_TABLE[nibble]))
        nibbles[i] = nibble
    return nibbles

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    mp3_path = os.path.join(base_dir, "assets", "music", "mayiiiii.mp3")
    if not os.path.exists(mp3_path):
        mp3_path = os.path.join(base_dir, "assets", "music", "mayiiiii.mp3.mpeg")
    output_bin = os.path.join(base_dir, "src", "audio", "mayiiiii_adpcm.bin")

    print(f"[*] Input MP3 File: {mp3_path} ({os.path.getsize(mp3_path)} bytes)")

    # 1. Decode MP3 via miniaudio
    print("[*] Decoding MP3 to 16-bit PCM...")
    audio = miniaudio.decode_file(mp3_path)
    s16 = np.frombuffer(audio.samples, dtype=np.int16).reshape(-1, 2)
    orig_frames = len(s16)
    orig_rate = audio.sample_rate
    orig_duration = orig_frames / orig_rate
    print(f"[+] Decoded: {orig_frames} frames, {orig_rate} Hz, 2 channels, duration: {orig_duration:.2f}s")

    # 2. Resample from 44,100 Hz to 24,000 Hz using Polyphase FIR (80 / 147)
    target_rate = 24000
    print(f"[*] Resampling to {target_rate} Hz via Polyphase FIR (ratio 80/147)...")
    left_24k = scipy.signal.resample_poly(s16[:, 0], 80, 147).astype(np.int16)
    right_24k = scipy.signal.resample_poly(s16[:, 1], 80, 147).astype(np.int16)
    resampled_frames = len(left_24k)
    print(f"[+] Resampled frames: {resampled_frames} at {target_rate} Hz")

    # 3. Align to 512-frame boundary (1 DMA descriptor = 512 frames @ 24 kHz = 1024 frames @ 48 kHz)
    pad = (512 - (resampled_frames % 512)) % 512
    if pad > 0:
        left_24k = np.pad(left_24k, (0, pad))
        right_24k = np.pad(right_24k, (0, pad))
    total_frames = len(left_24k)
    total_descriptors = total_frames // 512
    final_duration = total_frames / target_rate
    print(f"[+] Padded total frames: {total_frames} (aligned to {total_descriptors} DMA descriptors)")
    print(f"[+] Final continuous duration: {final_duration:.2f}s ({int(final_duration//60):02d}:{int(final_duration%60):02d})")

    # 4. Encode Left and Right channels into 4-bit IMA ADPCM
    print("[*] Compressing Left channel to 4-bit IMA ADPCM...")
    left_nibbles = encode_channel_adpcm(left_24k)
    print("[*] Compressing Right channel to 4-bit IMA ADPCM...")
    right_nibbles = encode_channel_adpcm(right_24k)

    # 5. Pack into bytes: (Left << 4) | Right
    print("[*] Packing stereo nibbles into byte stream (High=Left, Low=Right)...")
    packed_bytes = ((left_nibbles << 4) | (right_nibbles & 0x0F)).astype(np.uint8).tobytes()

    with open(output_bin, "wb") as f:
        f.write(packed_bytes)

    out_size = os.path.getsize(output_bin)
    print(f"[+] Successfully generated: {output_bin} ({out_size} bytes / {out_size / 1024 / 1024:.2f} MiB)")
    print(f"[+] Compression ratio: {orig_frames * 4 / out_size:.2f}:1 compared to raw 44.1 kHz 16-bit stereo PCM")

if __name__ == "__main__":
    main()
