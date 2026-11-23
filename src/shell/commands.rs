//! Shell Command Parser, Dispatcher, and Built-in Command Handlers.

use crate::console::read_line;
use crate::console::vga::{self, Color};
use crate::kprint;
use crate::kprintln;

/// Starts and runs the interactive shell loop.
pub fn run_shell() -> ! {
    kprintln!();
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("========================================================");
    kprintln!("         MICRO-OS INTERACTIVE COMMAND SHELL             ");
    kprintln!("========================================================");
    vga::set_color(Color::White, Color::Black);
    kprintln!("Type 'help' for available commands or 'run <program>'.");
    kprintln!();

    let mut line_buf = [0u8; 128];
    loop {
        vga::set_color(Color::LightCyan, Color::Black);
        kprint!("micro-os> ");
        vga::set_color(Color::White, Color::Black);
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
        "whoami" => cmd_whoami(),
        "hostname" => cmd_hostname(args),
        "uname" => cmd_uname(args),
        "list" => cmd_list(),
        "tasks" => cmd_tasks(),
        "mem" => cmd_mem(),
        "trace" => cmd_trace(),
        "run" => cmd_run(args),
        _ => {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("Unknown command: '{}'. Type 'help' for a list of commands.", cmd);
            vga::set_color(Color::White, Color::Black);
        }
    }
}

/// Handler for `help`: Displays all supported shell commands.
fn cmd_help() {
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("Micro-OS Shell Commands:");
    vga::set_color(Color::White, Color::Black);
    kprintln!("  help          - Display this help information");
    kprintln!("  clear         - Clear the screen buffer");
    kprintln!("  whoami        - Display current user identity");
    kprintln!("  hostname      - Display or set system hostname");
    kprintln!("  uname [-a]    - Display system and architecture information");
    kprintln!("  list          - List available Ring 3 user programs");
    kprintln!("  tasks         - Display active and registered kernel tasks and processes");
    kprintln!("  mem           - Display physical frame allocation and free-list statistics");
    kprintln!("  trace         - Dump diagnostics circular trace buffer");
    kprintln!("  run <program> - Execute a user program in isolated Ring 3 (hello, counter, calc)");
}

/// Handler for `clear`: Clears the VGA text screen and serial console.
fn cmd_clear() {
    vga::init_lavender_palette();
    vga::clear_screen();
    crate::serial_print!("\x1b[2J\x1b[H");
}

/// Handler for `whoami`: Displays the configured username.
fn cmd_whoami() {
    vga::set_color(Color::White, Color::Black);
    kprintln!("{}", crate::kernel::identity::get_username());
}

/// Handler for `hostname`: Displays or sets the system hostname.
fn cmd_hostname(args: &str) {
    if args.is_empty() {
        vga::set_color(Color::White, Color::Black);
        crate::kernel::identity::with_hostname(|name| {
            kprintln!("{}", name);
        });
    } else {
        match crate::kernel::identity::set_hostname(args) {
            Ok(()) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("Hostname updated to '{}'", args);
                vga::set_color(Color::Yellow, Color::Black);
                kprintln!("[NOTE: Runtime configuration only; resets on reboot]");
                vga::set_color(Color::White, Color::Black);
            }
            Err(err) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("Error setting hostname: {}", err);
                vga::set_color(Color::White, Color::Black);
            }
        }
    }
}

/// Handler for `uname`: Displays verified operating system and build information.
fn cmd_uname(args: &str) {
    let info = crate::kernel::identity::get_system_info();
    vga::set_color(Color::White, Color::Black);

    match args {
        "-a" => {
            crate::kernel::identity::with_hostname(|hostname| {
                kprintln!(
                    "{} {} {} {} {} (Milestone {})",
                    info.os_name,
                    hostname,
                    info.version,
                    info.architecture,
                    info.cpu_mode,
                    info.milestone
                );
            });
        }
        "-s" => {
            kprintln!("{}", info.os_name);
        }
        "-r" => {
            kprintln!("{}", info.version);
        }
        "-m" => {
            kprintln!("{}", info.architecture);
        }
        "-n" => {
            crate::kernel::identity::with_hostname(|hostname| {
                kprintln!("{}", hostname);
            });
        }
        "" => {
            kprintln!("Operating system: {}", info.os_name);
            kprintln!("Version:          {}", info.version);
            kprintln!("Architecture:     {}", info.architecture);
            kprintln!("CPU mode:         {}", info.cpu_mode);
            kprintln!("Milestone:        {}", info.milestone);
        }
        _ => {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("Usage: uname [-a | -s | -r | -m | -n]");
            vga::set_color(Color::White, Color::Black);
        }
    }
}

/// Handler for `list`: Lists registered Ring 3 user programs.
fn cmd_list() {
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("Available Ring 3 User Programs:");
    vga::set_color(Color::White, Color::Black);
    let progs = crate::task::get_user_programs();
    for p in progs {
        vga::set_color(Color::LightCyan, Color::Black);
        kprint!("  {:8}", p.name);
        vga::set_color(Color::White, Color::Black);
        kprintln!(" - {}", p.description);
    }
}

/// Handler for `tasks`: Displays kernel task manager table and process registry.
fn cmd_tasks() {
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Kernel Task Manager Registry ===");
    vga::set_color(Color::White, Color::Black);
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
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Process Registry ===");
    vga::set_color(Color::White, Color::Black);
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
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Physical Memory Frame Statistics ===");
    vga::set_color(Color::White, Color::Black);
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
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Kernel Diagnostics Trace Buffer ===");
    vga::set_color(Color::White, Color::Black);
    crate::kernel::diagnostics::TRACE_BUFFER.lock().dump();
}

/// Handler for `run <program>`: Loads and executes a user program in isolated Ring 3.
fn cmd_run(args: &str) {
    if args.is_empty() {
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("Usage: run <program>");
        kprintln!("Available programs: hello, counter, calc");
        vga::set_color(Color::White, Color::Black);
        return;
    }

    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("[SHELL] Launching user program '{}'...", args);
    vga::set_color(Color::White, Color::Black);
    match crate::task::run_user_program(args) {
        Ok(code) => {
            if code == 0 {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[SHELL] Program '{}' finished successfully with exit code {}.", args, code);
            } else {
                vga::set_color(Color::Yellow, Color::Black);
                kprintln!("[SHELL] Program '{}' finished with non-zero exit code {}.", args, code);
            }
            vga::set_color(Color::White, Color::Black);
        }
        Err(err) => {
            vga::set_color(Color::LightRed, Color::Black);
            kprintln!("[SHELL] Failed to execute '{}': {}", args, err);
            kprintln!("Type 'list' to see available programs.");
            vga::set_color(Color::White, Color::Black);
        }
    }
}
