//! Process abstraction, Process Registry, generation-safe Process IDs,
//! logical process memory partitions, and Inter-Process Communication (IPC).
//!
//! Part of Milestone 12: Process/Thread Research Specialization (Track G / Paper R6).

use core::fmt;
use alloc::alloc::{alloc, dealloc, Layout};
use spin::Mutex;
use crate::task::task::TaskId;

/// Maximum number of concurrent processes supported in the static registry.
pub const MAX_PROCESSES: usize = 16;

/// Maximum number of threads per process.
pub const MAX_THREADS_PER_PROCESS: usize = 4;

/// Maximum number of pending messages per IPC mailbox.
pub const IPC_MAILBOX_CAPACITY: usize = 8;

/// Standard logical process heap partition size: 16 KiB.
pub const PROCESS_PARTITION_SIZE: usize = 16 * 1024;

/// Strongly typed, generation-protected Process Identifier.
///
/// Under x86_64 System V ABI: `index` (u16) + 6 bytes padding + `generation` (u64) = 16 bytes, align 8.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId {
    pub index: u16,
    pub generation: u64,
}

impl ProcessId {
    pub const fn new(index: u16, generation: u64) -> Self {
        Self { index, generation }
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PID({}:g{})", self.index, self.generation)
    }
}

/// Life-cycle execution states of a kernel process.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Created,
    Running,
    Terminating,
    Zombie,
}

impl ProcessState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ProcessState::Created => "CREATED",
            ProcessState::Running => "RUNNING",
            ProcessState::Terminating => "TERMINATING",
            ProcessState::Zombie => "ZOMBIE",
        }
    }
}

/// An Inter-Process Communication (IPC) message payload.
///
/// Size: `sender: ProcessId` (16 B) + `len: usize` (8 B) + `data: [u8; 32]` (32 B) = 56 bytes, align 8.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct IpcMessage {
    pub sender: ProcessId,
    pub len: usize,
    pub data: [u8; 32],
}

impl IpcMessage {
    pub fn new(sender: ProcessId, payload: &[u8]) -> Self {
        let mut data = [0u8; 32];
        let len = payload.len().min(32);
        data[..len].copy_from_slice(&payload[..len]);
        Self { sender, len, data }
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

/// Fixed-capacity circular mailbox for a process.
///
/// Size: `messages: [Option<IpcMessage>; 8]` (512 B) + head/tail/count (24 B) = 536 bytes, align 8.
pub struct IpcMailbox {
    pub messages: [Option<IpcMessage>; IPC_MAILBOX_CAPACITY],
    pub head: usize,
    pub tail: usize,
    pub count: usize,
}

impl IpcMailbox {
    pub const fn new() -> Self {
        Self {
            messages: [None; IPC_MAILBOX_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, msg: IpcMessage) -> Result<(), IpcError> {
        if self.count >= IPC_MAILBOX_CAPACITY {
            return Err(IpcError::MailboxFull);
        }
        self.messages[self.tail] = Some(msg);
        self.tail = (self.tail + 1) % IPC_MAILBOX_CAPACITY;
        self.count += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<IpcMessage> {
        if self.count == 0 {
            return None;
        }
        let msg = self.messages[self.head].take();
        self.head = (self.head + 1) % IPC_MAILBOX_CAPACITY;
        self.count -= 1;
        msg
    }

    pub fn clear(&mut self) {
        for slot in self.messages.iter_mut() {
            *slot = None;
        }
        self.head = 0;
        self.tail = 0;
        self.count = 0;
    }
}

/// Process Control Block representing an isolated execution entity.
///
/// Size: 664 bytes, alignment 8 bytes.
pub struct ProcessDescriptor {
    pub pid: ProcessId,
    pub name: &'static str,
    pub state: ProcessState,
    pub threads: [Option<TaskId>; MAX_THREADS_PER_PROCESS],
    pub thread_count: usize,
    pub mailbox: IpcMailbox,
    pub partition_base: u64,
    pub partition_size: usize,
    pub cr3: Option<x86_64::structures::paging::PhysFrame>,
}

impl ProcessDescriptor {
    pub fn new(pid: ProcessId, name: &'static str, partition_base: u64, partition_size: usize) -> Self {
        Self {
            pid,
            name,
            state: ProcessState::Created,
            threads: [None; MAX_THREADS_PER_PROCESS],
            thread_count: 0,
            mailbox: IpcMailbox::new(),
            partition_base,
            partition_size,
            cr3: None,
        }
    }

    pub fn attach_address_space(&mut self, cr3: x86_64::structures::paging::PhysFrame) {
        self.cr3 = Some(cr3);
    }

    pub fn add_thread(&mut self, tid: TaskId) -> Result<(), ProcessError> {
        if self.thread_count >= MAX_THREADS_PER_PROCESS {
            return Err(ProcessError::ThreadLimitExceeded);
        }
        for slot in self.threads.iter_mut() {
            if slot.is_none() {
                *slot = Some(tid);
                self.thread_count += 1;
                return Ok(());
            }
        }
        Err(ProcessError::ThreadLimitExceeded)
    }

    pub fn remove_thread(&mut self, tid: TaskId) -> bool {
        for slot in self.threads.iter_mut() {
            if *slot == Some(tid) {
                *slot = None;
                self.thread_count = self.thread_count.saturating_sub(1);
                return true;
            }
        }
        false
    }

    pub fn has_thread(&self, tid: TaskId) -> bool {
        self.threads.iter().any(|slot| *slot == Some(tid))
    }
}

/// Persistent registry slot retaining its generation counter even when vacant.
///
/// Size: 672 bytes, alignment 8 bytes.
pub struct RegistrySlot {
    pub generation: u64,
    pub process: Option<ProcessDescriptor>,
}

impl RegistrySlot {
    pub const fn new() -> Self {
        Self {
            generation: 1, // Start with generation 1
            process: None,
        }
    }
}

/// Static Process Registry managing all process lifecycles.
///
/// Size: `processes: [RegistrySlot; 16]` (10,752 B) + `count: usize` (8 B) = 10,760 bytes, align 8.
pub struct ProcessRegistry {
    pub processes: [RegistrySlot; MAX_PROCESSES],
    pub count: usize,
}

impl ProcessRegistry {
    pub const fn new() -> Self {
        Self {
            processes: [const { RegistrySlot::new() }; MAX_PROCESSES],
            count: 0,
        }
    }

    /// Allocates a new process with an isolated logical memory partition.
    pub fn create_process(&mut self, name: &'static str) -> Result<ProcessId, ProcessError> {
        if self.count >= MAX_PROCESSES {
            return Err(ProcessError::ProcessTableFull);
        }

        // Find a vacant slot whose generation has not reached the retired sentinel
        let mut selected_slot = None;
        for (i, slot) in self.processes.iter_mut().enumerate() {
            if slot.process.is_none() && slot.generation < u64::MAX {
                selected_slot = Some((i, slot));
                break;
            }
        }

        let (idx, slot) = selected_slot.ok_or(ProcessError::ProcessTableFull)?;

        // Allocate 16 KiB heap memory partition for this process
        let layout = match Layout::from_size_align(PROCESS_PARTITION_SIZE, 16) {
            Ok(l) => l,
            Err(_) => return Err(ProcessError::PartitionAllocationFailed),
        };

        let partition_ptr = unsafe { alloc(layout) };
        if partition_ptr.is_null() {
            return Err(ProcessError::PartitionAllocationFailed);
        }

        let pid = ProcessId::new(idx as u16, slot.generation);
        let descriptor = ProcessDescriptor::new(pid, name, partition_ptr as u64, PROCESS_PARTITION_SIZE);

        slot.process = Some(descriptor);
        self.count += 1;
        Ok(pid)
    }

    /// Attaches an existing task/thread to a process.
    pub fn attach_thread(&mut self, pid: ProcessId, tid: TaskId) -> Result<(), ProcessError> {
        let desc = self.get_process_mut(pid)?;
        desc.add_thread(tid)
    }

    /// Removes a thread from a process. If thread count reaches 0, marks process Terminating.
    pub fn remove_thread(&mut self, pid: ProcessId, tid: TaskId) -> Result<(), ProcessError> {
        let desc = self.get_process_mut(pid)?;
        desc.remove_thread(tid);
        if desc.thread_count == 0 {
            desc.state = ProcessState::Terminating;
        }
        Ok(())
    }

    /// Immutable lookup validating exact PID generation.
    pub fn get_process(&self, pid: ProcessId) -> Result<&ProcessDescriptor, ProcessError> {
        let idx = pid.index as usize;
        if idx >= MAX_PROCESSES {
            return Err(ProcessError::ProcessNotFound);
        }
        let slot = &self.processes[idx];
        if slot.generation != pid.generation {
            return Err(ProcessError::StaleProcessId);
        }
        slot.process.as_ref().ok_or(ProcessError::ProcessNotFound)
    }

    /// Mutable lookup validating exact PID generation.
    pub fn get_process_mut(&mut self, pid: ProcessId) -> Result<&mut ProcessDescriptor, ProcessError> {
        let idx = pid.index as usize;
        if idx >= MAX_PROCESSES {
            return Err(ProcessError::ProcessNotFound);
        }
        let slot = &mut self.processes[idx];
        if slot.generation != pid.generation {
            return Err(ProcessError::StaleProcessId);
        }
        slot.process.as_mut().ok_or(ProcessError::ProcessNotFound)
    }

    /// Sends an IPC message from sender to target process.
    pub fn send_ipc(&mut self, sender: ProcessId, target: ProcessId, payload: &[u8]) -> Result<(), IpcError> {
        // Validate sender
        self.get_process(sender).map_err(|_| IpcError::InvalidSender)?;

        // Deliver to target mailbox
        let target_desc = self.get_process_mut(target).map_err(|e| match e {
            ProcessError::StaleProcessId => IpcError::StaleTarget,
            _ => IpcError::TargetNotFound,
        })?;

        let msg = IpcMessage::new(sender, payload);
        target_desc.mailbox.push(msg)
    }

    /// Receives the next available IPC message from a process's mailbox.
    pub fn recv_ipc(&mut self, receiver: ProcessId) -> Result<Option<IpcMessage>, IpcError> {
        let receiver_desc = self.get_process_mut(receiver).map_err(|_| IpcError::TargetNotFound)?;
        let msg_opt = receiver_desc.mailbox.pop();

        if let Some(msg) = msg_opt {
            // Validate sender generation: reject messages from reaped/stale sender incarnations
            let sender_valid = self.get_process(msg.sender).is_ok();
            if !sender_valid {
                return Err(IpcError::StaleSender);
            }
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }

    /// Reaps a terminating process: deallocates partition, drains mailbox, and increments generation.
    pub fn reap_process(&mut self, pid: ProcessId) -> Result<(), ProcessError> {
        let idx = pid.index as usize;
        if idx >= MAX_PROCESSES {
            return Err(ProcessError::ProcessNotFound);
        }

        let slot = &mut self.processes[idx];
        if slot.generation != pid.generation {
            return Err(ProcessError::StaleProcessId);
        }

        if let Some(mut desc) = slot.process.take() {
            // Drain mailbox
            desc.mailbox.clear();

            // Deallocate logical memory partition
            if desc.partition_base != 0 {
                let layout = Layout::from_size_align(desc.partition_size, 16)
                    .unwrap_or(Layout::new::<u8>());
                unsafe {
                    dealloc(desc.partition_base as *mut u8, layout);
                }
            }

            // Increment generation; if overflow, slot permanently retires at u64::MAX
            slot.generation = slot.generation.checked_add(1).unwrap_or(u64::MAX);
            self.count = self.count.saturating_sub(1);
            Ok(())
        } else {
            Err(ProcessError::ProcessNotFound)
        }
    }
}

/// Global Process Registry protected by a SpinMutex.
pub static PROCESS_REGISTRY: Mutex<ProcessRegistry> = Mutex::new(ProcessRegistry::new());

/// Error types for Process operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    ProcessTableFull,
    ProcessNotFound,
    StaleProcessId,
    ThreadLimitExceeded,
    PartitionAllocationFailed,
}

/// Error types for IPC operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    MailboxFull,
    TargetNotFound,
    StaleTarget,
    InvalidSender,
    StaleSender,
}

/// Public API helper to create a process.
pub fn create_process(name: &'static str) -> Result<ProcessId, ProcessError> {
    PROCESS_REGISTRY.lock().create_process(name)
}

/// Public API helper to attach a thread to a process.
pub fn attach_thread(pid: ProcessId, tid: TaskId) -> Result<(), ProcessError> {
    PROCESS_REGISTRY.lock().attach_thread(pid, tid)
}

/// Public API helper to send an IPC message.
pub fn sys_ipc_send(sender: ProcessId, target: ProcessId, data: &[u8]) -> Result<(), IpcError> {
    PROCESS_REGISTRY.lock().send_ipc(sender, target, data)
}

/// Public API helper to receive an IPC message.
pub fn sys_ipc_recv(receiver: ProcessId) -> Result<Option<IpcMessage>, IpcError> {
    PROCESS_REGISTRY.lock().recv_ipc(receiver)
}

/// Public API helper to reap a process.
pub fn reap_process(pid: ProcessId) -> Result<(), ProcessError> {
    PROCESS_REGISTRY.lock().reap_process(pid)
}
