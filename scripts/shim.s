.code16
.intel_syntax noprefix
.text
.globl _start

_start:
    jmp entry
    nop
    # 6 bytes padding to align boot info table at offset 8
    .byte 0, 0, 0, 0, 0, 0

# Offset 8 to 63: El Torito Boot Info Table (56 bytes)
# Patched by xorriso / mkisofs -boot-info-table:
# offset 8:  bi_pvd (32-bit PVD sector)
# offset 12: bi_file_lba (32-bit CD sector of cdboot.bin)
# offset 16: bi_length (32-bit file length in bytes)
# offset 20: bi_csum (32-bit checksum)
boot_info_table:
    .space 56, 0

entry:
    cli
    xor ax, ax
    mov ds, ax
    mov ss, ax
    mov sp, 0x1000       # Temporary stack below 0x1000
    sti

    # Relocate shim code (1024 bytes) from 0x0000:0x7C00 to 0x0100:0x0000 (phys 0x1000)
    cld
    mov si, 0x7C00
    mov ax, 0x0100
    mov es, ax
    xor di, di
    mov cx, 1024
    rep movsb

    # Far jump to relocated code at 0x0100:relocated_entry
    push 0x0100
    push offset relocated_entry
    retf

relocated_entry:
    # Now executing in relocated memory at segment 0x0100 (CS=0x0100, DS=0x0100)!
    mov ax, cs
    mov ds, ax
    mov es, ax

    # Save boot drive from BIOS (DL)
    mov byte ptr [boot_drive], dl

    # Read CD LBA of cdboot.bin from Boot Info Table (offset 12 = 0x0000:0x7C0C)
    xor ax, ax
    mov fs, ax
    mov eax, dword ptr fs:[0x7C0C]
    # bootimage.bin starts exactly 1 CD sector (2048 bytes) after cdboot.bin!
    inc eax
    mov dword ptr [bootimage_cd_lba], eax

    cli
    # Save original INT 13h vector from IVT (0x0000:0x004C)
    mov ax, word ptr fs:[0x004C]
    mov word ptr cs:[orig_int13_ip], ax
    mov ax, word ptr fs:[0x004E]
    mov word ptr cs:[orig_int13_cs], ax

    # Install our INT 13h handler
    mov word ptr fs:[0x004C], offset int13_handler
    mov word ptr fs:[0x004E], cs
    sti

    # Initialize bounce buffer cache status
    mov dword ptr cs:[cached_cd_lba], 0xFFFFFFFF

    # Load Sector 0 of bootimage.bin (stage 1, 512 bytes) to 0x7C00
    sub sp, 16
    mov bp, sp
    mov byte ptr [bp+0], 16
    mov byte ptr [bp+1], 0
    mov word ptr [bp+2], 1       # 1 block (512 bytes)
    mov word ptr [bp+4], 0x7C00   # offset
    mov word ptr [bp+6], 0x0000   # segment
    mov dword ptr [bp+8], 0       # LBA low = 0
    mov dword ptr [bp+12], 0      # LBA high = 0

    mov si, bp
    mov ax, ss
    mov ds, ax
    call read_blocks_internal
    add sp, 16

    # Restore ds to cs (0x0100)
    mov ax, cs
    mov ds, ax

    # Check magic at 0x0000:0x7DFE
    xor ax, ax
    mov fs, ax
    cmp word ptr fs:[0x7DFE], 0xAA55
    je .magic_ok

    # Magic failed: print 'M' and halt
    mov al, 'M'
    mov ah, 0x0E
    int 0x10
.spin_magic:
    hlt
    jmp .spin_magic

.magic_ok:
    # Restore segments to 0 for stage 1
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov fs, ax

    # Pass boot drive in DL (stage 1 expects DL)
    mov dl, byte ptr cs:[boot_drive]

    # Initialize stack at 0x7C00 for stage 1
    mov sp, 0x7C00

    # Jump to stage 1 at 0x0000:0x7C00!
    jmp 0x0000:0x7C00

# ---------------------------------------------------------------------------
# INT 13h Handler (Hooks BIOS Disk Services)
# ---------------------------------------------------------------------------
int13_handler:
    # Check if request targets our boot drive or drive 0x80
    cmp dl, byte ptr cs:[boot_drive]
    je .match
    cmp dl, 0x80
    je .match

    # Chain to original BIOS handler
    jmp dword ptr cs:[orig_int13_ip]

.match:
    cmp ah, 0x41
    je .handle_check_ext
    cmp ah, 0x42
    je .handle_ext_read
    cmp ah, 0x00
    je .handle_reset
    cmp ah, 0x08
    je .handle_params
    cmp ah, 0x48
    je .handle_params_ext

    # Other functions: chain
    jmp dword ptr cs:[orig_int13_ip]

.handle_reset:
    push bp
    mov bp, sp
    and word ptr [bp+6], 0xFFFE   # Clear CF in pushed flags
    pop bp
    xor ah, ah
    iret

.handle_check_ext:
    cmp bx, 0x55AA
    jne .chain_orig
    mov bx, 0xAA55
    mov cx, 0x0001               # Fixed disk access subset
    mov ah, 0x30                 # EDD 3.0
    push bp
    mov bp, sp
    and word ptr [bp+6], 0xFFFE   # Clear CF
    pop bp
    iret

.chain_orig:
    jmp dword ptr cs:[orig_int13_ip]

.handle_params:
    push bp
    mov bp, sp
    and word ptr [bp+6], 0xFFFE   # Clear CF
    pop bp
    xor ax, ax
    mov ch, 0xFF                 # Cylinders
    mov cl, 0x3F                 # Sectors
    mov dh, 15                   # Heads
    mov dl, 1
    xor di, di
    mov es, di
    iret

.handle_params_ext:
    push bp
    mov bp, sp
    and word ptr [bp+6], 0xFFFE   # Clear CF
    pop bp
    xor ah, ah
    iret

.handle_ext_read:
    # DS:SI points to caller DAP
    call read_blocks_internal
    jc .read_failed

    push bp
    mov bp, sp
    and word ptr [bp+6], 0xFFFE   # Clear CF
    pop bp
    xor ah, ah
    iret

.read_failed:
    push bp
    mov bp, sp
    or word ptr [bp+6], 1         # Set CF
    pop bp
    mov ah, 0x04                 # Read error
    iret

# ---------------------------------------------------------------------------
# Internal Read Blocks Routine
# In: DS:SI points to DAP (16 bytes)
# Out: CF=0 on success, CF=1 on failure
# ---------------------------------------------------------------------------
read_blocks_internal:
    pusha
    push ds
    push es

    mov cx, word ptr [si+2]       # Block count
    test cx, cx
    jz .done_ok

    mov bx, word ptr [si+4]       # Dest offset
    mov dx, word ptr [si+6]       # Dest segment
    mov eax, dword ptr [si+8]     # Starting 512-byte LBA

.block_loop:
    push cx
    push eax
    push dx
    push bx

    # Compute CD LBA: cd_lba = bootimage_cd_lba + (eax >> 2)
    # Compute sub_block: sub_block = eax & 3
    mov edx, eax
    and edx, 3                    # sub_block index (0, 1, 2, or 3)
    shr eax, 2                    # 512-byte LBA to 2048-byte CD sector
    add eax, dword ptr cs:[bootimage_cd_lba]

    # Check cache
    cmp eax, dword ptr cs:[cached_cd_lba]
    je .copy_block

    # Read CD sector from CD-ROM drive into bounce buffer (0x1400)
    push edx                      # Save sub_block
    push eax                      # Save CD LBA

    sub sp, 16
    mov bp, sp
    mov byte ptr [bp+0], 16
    mov byte ptr [bp+1], 0
    mov word ptr [bp+2], 1       # 1 CD sector (2048 bytes)
    mov word ptr [bp+4], 0x0400   # Bounce buffer offset in 0x0100
    mov word ptr [bp+6], 0x0100   # Bounce buffer segment (0x0100 * 16 + 0x0400 = 0x1400)
    mov dword ptr [bp+8], eax     # CD LBA low
    mov dword ptr [bp+12], 0      # CD LBA high

    xor ax, ax
    mov ds, ax
    mov si, bp
    mov dl, byte ptr cs:[boot_drive]
    mov ah, 0x42

    pushf
    call dword ptr cs:[orig_int13_ip]
    jc .read_cd_failed

    add sp, 16
    pop eax
    pop edx

    # Cache successful read
    mov dword ptr cs:[cached_cd_lba], eax
    jmp .copy_block

.read_cd_failed:
    add sp, 16
    pop eax
    pop edx
    jmp .io_error

.copy_block:
    # Source offset in bounce buffer = 0x0400 + (sub_block * 512)
    shl dx, 9                     # dx = sub_block * 512
    add dx, 0x0400                # dx = source offset
    mov si, dx
    mov ax, cs
    mov ds, ax                    # Source seg = 0x0100

    mov bp, sp
    mov di, word ptr [bp+0]       # Dest offset
    mov es, word ptr [bp+2]       # Dest segment

    cld
    mov cx, 256                   # 256 words = 512 bytes
    rep movsw

    # Advance dest segment by 0x20 (32 paragraphs * 16 = 512 bytes)
    add word ptr [bp+2], 0x20

    pop bx
    pop dx
    pop eax
    pop cx

    inc eax                       # Next 512-byte LBA
    dec cx
    jnz .block_loop

.done_ok:
    pop es
    pop ds
    popa
    clc
    ret

.io_error:
    pop bx
    pop dx
    pop eax
    pop cx
    pop es
    pop ds
    popa
    stc
    ret

# ---------------------------------------------------------------------------
# Data Area (relocated together with code to 0x1000)
# ---------------------------------------------------------------------------
boot_drive:        .byte 0
bootimage_cd_lba:  .long 0
cached_cd_lba:     .long 0xFFFFFFFF
orig_int13_ip:     .word 0
orig_int13_cs:     .word 0

# Pad to exactly 2048 bytes (1 CD sector)
.org 2048
