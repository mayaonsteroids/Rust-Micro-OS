//! Interactive Command-Line Shell Subsystem for Rust Micro-OS.
//!
//! Provides a user-interactive terminal interface running directly on bare metal:
//! - Dual input handling via COM1 Serial and PS/2 Keyboard.
//! - Prompt `micro-os> ` with character echo, backspace line editing, and history.
//! - Built-in commands: `help`, `clear`, `list`, `tasks`, `mem`, `trace`.
//! - Program execution command: `run <program>` (supporting `hello`, `counter`, `calc`).
//! - Hardware-isolated Ring 3 execution with safe kernel return and zero memory leaks.

pub mod commands;

pub use commands::{dispatch_command, run_shell};
