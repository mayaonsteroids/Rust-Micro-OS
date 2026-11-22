# Micro-OS Architecture Document

## Subsystem Architecture

```text
+-------------------------------------------------------------+
|              Micro-OS Demonstration & Tests                 |
+-------------------------------------------------------------+
|        Research Specialization: Process vs Thread Bench     |
+-------------------------------------------------------------+
|              Round Robin Preemptive Scheduler               |
+-------------------------------------------------------------+
|              Task Management & Ready Queue                  |
+-------------------------------------------------------------+
|        Synchronization (Spinlock) & Shared Resources        |
+-------------------------------------------------------------+
|              Memory Allocator & Heap Tracking               |
+-------------------------------------------------------------+
|           Interrupt Layer (IDT, PIC 8259, PIT Timer)        |
+-------------------------------------------------------------+
|           Console (VGA 0xb8000) & Logger (COM1 0x3F8)       |
+-------------------------------------------------------------+
|                    Hardware Abstraction                     |
+-------------------------------------------------------------+
|             Bootloader (x86_64 Long Mode Entry)             |
+-------------------------------------------------------------+
|                    QEMU / Physical Hardware                 |
+-------------------------------------------------------------+
```

## Control and Data Flow
1. **Bootloader Phase:** BIOS initializes real mode, enters 32-bit protected mode, enables paging and long mode (64-bit), and calls `_start`.
2. **Initialization Phase:** Hardware interrupt controller (PIC) and timer (PIT) are set up; VGA text and serial UART are opened.
3. **Execution Phase:** Tasks are allocated, placed on the Ready Queue, and scheduled in Round Robin cycles driven by timer interrupts.
4. **Research Phase:** Thread-like tasks and process-like tasks are benchmarked for creation and execution overhead.
