//! Kernel Diagnostics, Hardware Telemetry, and Debugging Trace Engine (Milestone 14).
//!
//! Provides comprehensive observation, instrumentation, and trace mechanisms conforming to
//! CA-6 Section 15 (Testing Strategy), Section 16 (Required Test Cases), Section 17 (Evidence),
//! Section 18 (Performance Evaluation), and Section 21 (Debugging Strategy).

use core::sync::atomic::Ordering;
use crate::{info, debug};

/// Architectural register snapshot captured for debugging and crash telemetry.
#[derive(Debug, Clone, Copy)]
pub struct RegisterDump {
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub rflags: u64,
}

impl RegisterDump {
    /// Captures the current CPU control registers and architectural status flags.
    pub fn capture() -> Self {
        use x86_64::registers::control::{Cr0, Cr2, Cr3, Cr4};
        use x86_64::registers::rflags;

        let cr0 = Cr0::read_raw();
        let cr2 = Cr2::read().as_u64();
        let (cr3_frame, _) = Cr3::read();
        let cr3 = cr3_frame.start_address().as_u64();
        let cr4 = Cr4::read_raw();
        let rflags = rflags::read_raw();

        Self {
            cr0,
            cr2,
            cr3,
            cr4,
            rflags,
        }
    }

    /// Emits a structured diagnostic log of captured CPU register state.
    pub fn log(&self) {
        info!("--- CPU Architectural Register Snapshot ---");
        info!("  CR0: 0x{:016x} (Paging, Protected Mode, WP)", self.cr0);
        info!("  CR2: 0x{:016x} (Page Fault Linear Address)", self.cr2);
        info!("  CR3: 0x{:016x} (PML4 Base Address)", self.cr3);
        info!("  CR4: 0x{:016x} (PAE, OSFXSR, OSXMMEXCPT)", self.cr4);
        info!("  RFLAGS: 0x{:016x} (IOPL, IF, Reserved)", self.rflags);
    }
}

/// Comprehensive kernel telemetry metrics reflecting subsystem states.
#[derive(Debug, Clone, Copy, Default)]
pub struct KernelTelemetry {
    pub registered_tasks: usize,
    pub active_tasks: usize,
    pub ready_queue_len: usize,
    pub heap_allocated_bytes: usize,
    pub heap_freed_bytes: usize,
    pub heap_current_used: usize,
    pub active_cr3: u64,
    pub page_faults: u64,
    pub gpf_count: u64,
    pub syscall_count: u64,
}

impl KernelTelemetry {
    /// Collects live telemetry metrics across all kernel subsystems without deadlock.
    pub fn collect() -> Self {
        let (registered_tasks, active_tasks) = {
            let tm = crate::task::TASK_MANAGER.lock();
            (tm.count(), tm.active_count())
        };

        let ready_queue_len = {
            crate::task::READY_QUEUE.lock().len()
        };

        let metrics = crate::memory::allocator::get_metrics();
        let (heap_allocated_bytes, heap_freed_bytes, heap_current_used) = (
            metrics.allocated_bytes,
            metrics.deallocated_bytes,
            metrics.used_bytes,
        );

        let active_cr3 = {
            let (frame, _) = x86_64::registers::control::Cr3::read();
            frame.start_address().as_u64()
        };

        let page_faults = crate::interrupts::idt::USER_FAULT_COUNT.load(Ordering::SeqCst);
        let gpf_count = crate::interrupts::idt::USER_GPF_COUNT.load(Ordering::SeqCst);
        let syscall_count = crate::interrupts::syscall::TOTAL_SYSCALL_COUNT.load(Ordering::SeqCst);

        Self {
            registered_tasks,
            active_tasks,
            ready_queue_len,
            heap_allocated_bytes,
            heap_freed_bytes,
            heap_current_used,
            active_cr3,
            page_faults,
            gpf_count,
            syscall_count,
        }
    }

    /// Emits a structured telemetry log summary.
    pub fn log(&self) {
        info!("--- Kernel Telemetry & Subsystem Diagnostics ---");
        info!("  Tasks: Registered={} | Active={} | ReadyQueue Depth={}",
            self.registered_tasks, self.active_tasks, self.ready_queue_len);
        info!("  Memory: HeapAllocated={} B | HeapFreed={} B | CurrentUsage={} B",
            self.heap_allocated_bytes, self.heap_freed_bytes, self.heap_current_used);
        info!("  MMU & Hardware: Active CR3=0x{:x}", self.active_cr3);
        info!("  Exceptions & Traps: #PF={} | #GP={} | Syscalls Dispatched={}",
            self.page_faults, self.gpf_count, self.syscall_count);
    }
}

/// Enumeration of distinct kernel trace events for the circular event tracer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventKind {
    TaskCreated,
    TaskDispatched,
    TaskYielded,
    TaskTerminated,
    SyscallDispatched,
    PageFaultTriggered,
    MemoryAllocated,
    MemoryFreed,
    LockContended,
    LockReleased,
}

impl TraceEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TraceEventKind::TaskCreated => "TASK_CREATED",
            TraceEventKind::TaskDispatched => "TASK_DISPATCHED",
            TraceEventKind::TaskYielded => "TASK_YIELDED",
            TraceEventKind::TaskTerminated => "TASK_TERMINATED",
            TraceEventKind::SyscallDispatched => "SYSCALL",
            TraceEventKind::PageFaultTriggered => "PAGE_FAULT",
            TraceEventKind::MemoryAllocated => "MEM_ALLOC",
            TraceEventKind::MemoryFreed => "MEM_FREE",
            TraceEventKind::LockContended => "LOCK_CONTENDED",
            TraceEventKind::LockReleased => "LOCK_RELEASED",
        }
    }
}

/// A recorded kernel trace event.
#[derive(Debug, Clone, Copy)]
pub struct TraceEvent {
    pub tick: u64,
    pub kind: TraceEventKind,
    pub data0: u64,
    pub data1: u64,
}

const TRACE_BUFFER_CAPACITY: usize = 32;

/// Fixed-size circular buffer for recording recent execution events for post-mortem debugging.
pub struct KernelTraceBuffer {
    buffer: [Option<TraceEvent>; TRACE_BUFFER_CAPACITY],
    head: usize,
    count: usize,
}

impl KernelTraceBuffer {
    pub const fn new() -> Self {
        Self {
            buffer: [None; TRACE_BUFFER_CAPACITY],
            head: 0,
            count: 0,
        }
    }

    /// Records an event into the circular buffer.
    pub fn record(&mut self, kind: TraceEventKind, data0: u64, data1: u64) {
        let tick = crate::timer::get_ticks();
        let event = TraceEvent {
            tick,
            kind,
            data0,
            data1,
        };

        self.buffer[self.head] = Some(event);
        self.head = (self.head + 1) % TRACE_BUFFER_CAPACITY;
        if self.count < TRACE_BUFFER_CAPACITY {
            self.count += 1;
        }
    }

    /// Returns the number of recorded events in the ring buffer.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Dumps recorded trace history to the kernel diagnostic log.
    pub fn dump(&self) {
        info!("--- Circular Execution Trace Buffer (Last {} events) ---", self.count);
        if self.count == 0 {
            info!("  [Trace Buffer Empty]");
            return;
        }

        let start = if self.count < TRACE_BUFFER_CAPACITY {
            0
        } else {
            self.head
        };

        for i in 0..self.count {
            let idx = (start + i) % TRACE_BUFFER_CAPACITY;
            if let Some(event) = self.buffer[idx] {
                debug!("  [Tick {:06}] {:<16} Data0=0x{:x} Data1=0x{:x}",
                    event.tick, event.kind.as_str(), event.data0, event.data1);
            }
        }
    }
}

/// Global trace buffer protected by an interrupt-safe spinlock.
pub static TRACE_BUFFER: crate::sync::IrqSpinLock<KernelTraceBuffer> =
    crate::sync::IrqSpinLock::new(KernelTraceBuffer::new());

/// Public helper to record an execution trace event without deadlocks.
pub fn trace_event(kind: TraceEventKind, data0: u64, data1: u64) {
    TRACE_BUFFER.lock().record(kind, data0, data1);
}
