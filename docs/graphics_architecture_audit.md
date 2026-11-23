# Micro-OS Graphics Boot Architecture Audit & Decision

**Project:** Rust Micro-OS  
**Branch:** `milestone-17-network-browser`  
**Checkpoint Commit:** `0bfb1b3`  
**Backup Branch:** `backup-pre-pixel-gui-0bfb1b3`  
**Target:** Genuine Pixel-Based Graphical Desktop (1024×768 / 800×600 @ 32 bpp Truecolor)

---

## 1. Codebase & Bootloader Source Audit

### Current Boot Chain
1. **El Torito CD-ROM Sector (2048 bytes):** BIOS loads `scripts/shim.s` (`cdboot.bin`) in 16-bit Real Mode at `0x0000:0x7C00`.
2. **Shim Relocation & Hooking:** `shim.s` relocates itself to segment `0x0100:0x0000`, hooks BIOS `INT 13h` to provide cached 2048-byte CD sector reads mapped as 512-byte virtual disk sectors, and loads `bootloader` Stage 1 to `0x7C00`.
3. **Rust `bootloader` 0.9.23 / 0.9.35 Execution:**
   - Stage 1 $\rightarrow$ Stage 2 $\rightarrow$ Stage 3 $\rightarrow$ Protected Mode $\rightarrow$ Long Mode.
   - Stage 2 (`src/stage_2.s:89`) executes:
     ```assembly
     call config_video_mode
     ```
     which is linked to `src/video_mode/vga_text_80x25.s`:
     ```assembly
     config_video_mode:
         mov ah, 0
         mov al, 0x03    # 80x25 16-color text mode
         int 0x10
         ret
     ```
   - In Long Mode, `bootloader` creates page tables, maps physical memory up to the top of usable RAM (`boot_info.physical_memory_offset`), and invokes `kernel_main(boot_info)`.
   - Inspection of `bootloader::bootinfo::BootInfo` reveals only:
     - `memory_map: MemoryMap`
     - `physical_memory_offset: u64`
     - `recursive_page_table_addr: u64`
     - **No framebuffer pointer, pitch, or dimensions exist in `BootInfo` v0.9.x.**

---

## 2. Evaluation of Architectural Options

### Option A: VESA BIOS Extensions (VBE) in `shim.s` (Real Mode)
* **Mechanism:** Call BIOS `INT 10h, AX=0x4F02, BX=0x4118` inside `shim.s` before loading `bootloader` Stage 1, record `PhysBasePtr` in a low-memory scratch area.
* **Fatal Limitation:** As identified above, `bootloader 0.9.x` Stage 2 unconditionally invokes `INT 10h, AL=0x03` during its transition to Protected Mode, actively overwriting any real-mode VBE mode back to 80×25 text mode.
* **Assessment:** Incompatible with `bootloader 0.9.x` without maintaining a custom, privately maintained fork of the `bootloader` crate on crates.io.

### Option B: Bootloader Replacement (Limine)
* **Mechanism:** Replace `bootloader 0.9.x` with Limine (or Multiboot2/GRUB) to receive a linear framebuffer structure at kernel entry.
* **Impact & Risk:**
  - *Integration Effort:* Very High.
  - *Breakage Risk:* Extreme. The existing entry point (`entry_point!(kernel_main)`), memory map translation (`BootInfoFrameAllocator`), page table initialization (`init_offset_page_table`), task context switching, and cargo build system (`cargo bootimage`) are deeply coupled to the `bootloader 0.9.x` ABI.
  - Overwriting this foundation risks breaking Milestone 11–17 subsystems (E1000 networking, AC97 DMA audio streaming, task scheduler, interrupts).
* **Assessment:** Unnecessarily risky when safer, native bare-metal options exist.

### Option C (Selected Architecture): Native Display Controller Driver via PCI and Bochs/VirtualBox BGA (Bochs Graphics Adapter)
* **Mechanism:**
  In both Oracle VirtualBox and QEMU, the display hardware is exposed directly on the PCI bus:
  1. **PCI Discovery:** The kernel already contains a functional PCI bus driver (`src/pci/`). Probing for Class `0x03` (Display Controller), Subclass `0x00` (VGA Compatible Controller) identifies the graphics device:
     - In QEMU: Vendor `0x1234`, Device `0x1111` (Bochs VGA / QEMU Standard VGA).
     - In VirtualBox: Vendor `0x80ee`, Device `0xbeef` (VirtualBox Graphics Adapter) or `0x1234:0x1111`.
  2. **Physical Framebuffer Address:** Read directly from PCI **BAR0** (`0xfd000000` in QEMU, `0xe0000000` in VirtualBox). The physical video RAM is typically 16 MiB to 128 MiB.
  3. **Hardware Mode Programming via Standard BGA I/O Ports:**
     Both QEMU and VirtualBox (with standard VBoxVGA/VBoxSVGA) implement the Bochs VBE Display Interface at I/O ports `0x01CE` (index) and `0x01CF` (data):
     - Index 0 (`ID`): Verify hardware presence (`0xB0C0`..`0xB0C5`).
     - Index 1 (`XRES`): Set horizontal resolution (`1024` or `800`).
     - Index 2 (`YRES`): Set vertical resolution (`768` or `600`).
     - Index 3 (`BPP`): Set 32-bit Truecolor (`32`).
     - Index 4 (`ENABLE`): Write `0x41` (`VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED`).
  4. **Virtual Memory Mapping:**
     Our kernel already contains `crate::memory::paging::map_mmio_range(PhysAddr::new(bar0), size)` which dynamically maps physical MMIO pages into the Level 4 Page Table with `PRESENT | WRITABLE | NO_CACHE` flags.
  5. **Direct Pixel Rendering:**
     Writing to `fb_virt + (y * pitch + x * 4)` directly updates VRAM pixels with zero heap allocation.

---

## 3. Comparative Architecture Matrix

| Criterion | Option A (Real-Mode VBE) | Option B (Limine Replacement) | Option C (PCI + BGA Driver) [SELECTED] |
| :--- | :--- | :--- | :--- |
| **Legacy BIOS Compatibility** | Broken by `bootloader 0.9` | Fully Supported | **100% Compatible (Untouched)** |
| **VirtualBox Compatibility** | Poor (text reset) | Good | **100% Native (VBoxVGA / VBoxSVGA)** |
| **QEMU Compatibility** | Poor (text reset) | Good | **100% Native (stdvga)** |
| **Bootloader Modification** | Requires crate fork | Complete kernel rewrite | **Zero changes to bootloader** |
| **Build Chain Impact** | High | Replaces `cargo bootimage` | **Zero (`cargo bootimage` preserved)** |
| **Existing OS Subsystem Risk** | Medium | Extreme | **Zero (Completely Isolated)** |
| **Resolution & Bit Depth** | 1024×768 @ 32 bpp | Configurable | **1024×768 / 800×600 @ 32 bpp Truecolor** |
| **Heap Memory Impact** | Zero | Minimal | **Zero (VRAM MMIO mapped)** |
| **Rollback Safety** | Medium | Difficult | **Trivial (Instantaneous)** |

---

## 4. Decision & Implementation Plan

**Decision:** Proceed with **Option C (Native Display Controller Driver via PCI and Bochs/VirtualBox BGA)** as the primary display engine.

### Subsystem Structure
```
src/graphics/
├── mod.rs           # Driver initialization, mode switching, public API
├── color.rs         # 32-bit RGBA/BGRA truecolor types, Light Lavender palette
├── framebuffer.rs   # Hardware BGA port I/O, PCI BAR0 detection, MMIO mapping
├── font.rs          # 8×16 bitmap font table (256 ASCII glyphs) & string rendering
├── draw.rs          # Rectangles, filled boxes, lines, shadows, clipping bounds
└── image.rs         # 32×32 pixel bitmap application icons with alpha blending
```

### Rollback Plan
A clean checkpoint branch `backup-pre-pixel-gui-0bfb1b3` has been created at commit `0bfb1b3`. The development ISO backup `iso/micro_os_text_backup.iso` is preserved. If any hardware issue arises, `git checkout backup-pre-pixel-gui-0bfb1b3` instantly restores the exact working system.
