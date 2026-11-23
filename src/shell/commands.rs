//! Shell Command Parser, Dispatcher, and Built-in Command Handlers.

use crate::console::read_line;
use crate::kprint;
use crate::kprintln;

/// Starts and runs the interactive shell loop.
pub fn run_shell() -> ! {
    kprintln!();
    kprintln!("========================================================");
    kprintln!("         MICRO-OS INTERACTIVE COMMAND SHELL             ");
    kprintln!("========================================================");
    kprintln!("Type 'help' for available commands or 'run <program>'.");
    kprintln!();

    let mut line_buf = [0u8; 128];
    loop {
        kprint!("micro-os> ");
        let len = read_line(&mut line_buf);
        if len == 0 {
            continue;
        }

        if let Ok(line) = core::str::from_utf8(&line_buf[..len]) {
            let line = line.trim();
            if !line.is_empty() {
                dispatch_command(line);
            }
        }
    }
}

/// Parses an input command string and dispatches to the corresponding handler.
pub fn dispatch_command(input: &str) {
    let mut parts = input.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").trim();
    let args = parts.next().unwrap_or("").trim();

    match cmd {
        "help" => cmd_help(),
        "clear" => cmd_clear(),
        "list" => cmd_list(),
        "tasks" => cmd_tasks(),
        "mem" => cmd_mem(),
        "trace" => cmd_trace(),
        "run" => cmd_run(args),
        _ => {
            kprintln!("Unknown command: '{}'. Type 'help' for a list of commands.", cmd);
        }
    }
}

/// Handler for `help`: Displays all supported shell commands.
fn cmd_help() {
    kprintln!("Micro-OS Shell Commands:");
    kprintln!("  help          - Display this help information");
    kprintln!("  clear         - Clear the screen buffer");
    kprintln!("  list          - List available Ring 3 user programs");
    kprintln!("  tasks         - Display active and registered kernel tasks and processes");
    kprintln!("  mem           - Display physical frame allocation and free-list statistics");
    kprintln!("  trace         - Dump diagnostics circular trace buffer");
    kprintln!("  run <program> - Execute a user program in isolated Ring 3 (hello, counter, calc)");
}

/// Handler for `clear`: Clears the VGA text screen and serial console.
fn cmd_clear() {
    crate::console::vga::clear_screen();
    crate::serial_print!("\x1b[2J\x1b[H");
}

/// Handler for `list`: Lists registered Ring 3 user programs.
fn cmd_list() {
    kprintln!("Available Ring 3 User Programs:");
    let progs = crate::task::get_user_programs();
    for p in progs {
        kprintln!("  {:8} - {}", p.name, p.description);
    }
}

/// Handler for `tasks`: Displays kernel task manager table and process registry.
fn cmd_tasks() {
    kprintln!("=== Kernel Task Manager Registry ===");
    let tm = crate::task::TASK_MANAGER.lock();
    kprintln!("Total registered tasks: {}", tm.count());
    kprintln!("TID  NAME           PRIORITY    STATE");
    kprintln!("---------------------------------------");
    for slot in tm.tasks().iter() {
        if let Some(task) = slot {
            kprintln!(
                "{:3}  {:14} {:10?} {:?}",
                task.id,
                task.name,
                task.priority,
                task.state
            );
        }
    }
    drop(tm);

    kprintln!();
    kprintln!("=== Process Registry ===");
    let pr = crate::task::PROCESS_REGISTRY.lock();
    kprintln!("Active processes: {} / {}", pr.count, crate::task::MAX_PROCESSES);
    kprintln!("PID        NAME           STATE        THREADS");
    kprintln!("------------------------------------------------");
    for slot in pr.processes.iter() {
        if let Some(ref proc) = slot.process {
            kprintln!(
                "{:10} {:14} {:12} {}",
                proc.pid,
                proc.name,
                proc.state.as_str(),
                proc.thread_count
            );
        }
    }
}

/// Handler for `mem`: Displays physical frame memory allocation metrics and recycling status.
fn cmd_mem() {
    kprintln!("=== Physical Memory Frame Statistics ===");
    if let Some(stats) = crate::memory::get_memory_stats() {
        kprintln!("  Allocated frames:   {}", stats.allocated_frames);
        kprintln!("  Deallocated frames: {}", stats.deallocated_frames);
        kprintln!("  Recycled in pool:   {} / {}", stats.recycled_frames, stats.capacity);
        let active = stats.allocated_frames.saturating_sub(stats.deallocated_frames);
        kprintln!("  Active frames:      {} ({} KiB)", active, active * 4);
    } else {
        kprintln!("  Frame allocator not initialized.");
    }
}

/// Handler for `trace`: Dumps the kernel diagnostics circular trace ring buffer.
fn cmd_trace() {
    kprintln!("=== Kernel Diagnostics Trace Buffer ===");
    crate::kernel::diagnostics::TRACE_BUFFER.lock().dump();
}

/// Handler for `run <program>`: Loads and executes a user program in isolated Ring 3.
fn cmd_run(args: &str) {
    if args.is_empty() {
        kprintln!("Usage: run <program>");
        kprintln!("Available programs: hello, counter, calc");
        return;
    }

    kprintln!("[SHELL] Launching user program '{}'...", args);
    match crate::task::run_user_program(args) {
        Ok(code) => {
            kprintln!("[SHELL] Program '{}' finished successfully with exit code {}.", args, code);
        }
        Err(err) => {
            kprintln!("[SHELL] Failed to execute '{}': {}", args, err);
            kprintln!("Type 'list' to see available programs.");
        }
    }
}
