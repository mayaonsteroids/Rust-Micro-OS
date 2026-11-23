#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

// Core subsystem modules for the educational Micro-OS
pub mod browser;
pub mod console;
pub mod interrupts;
pub mod kernel;
pub mod memory;
pub mod net;
pub mod pci;
pub mod research;
pub mod scheduler;
pub mod shell;
pub mod sync;
pub mod task;
pub mod timer;
