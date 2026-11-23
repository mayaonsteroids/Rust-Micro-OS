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
        "pci" => cmd_pci(),
        "net" => cmd_net(args),
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
    kprintln!("  pci           - Enumerate and display PCI bus devices and BARs");
    kprintln!("  net [test]    - Display network controller status, MAC, and test packet");
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

/// Handler for `pci`: Scans and displays active PCI bus devices.
fn cmd_pci() {
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== PCI Bus Device Registry ===");
    vga::set_color(Color::White, Color::Black);

    let devices = crate::pci::scan_pci();
    kprintln!("Total active PCI devices: {}", devices.len());
    kprintln!("BUS:SLOT.F  VENDOR:DEV   CLASS  SUB  NAME");
    kprintln!("------------------------------------------------------------");

    for dev in &devices {
        kprintln!(
            "{:02x}:{:02x}.{}    0x{:04x}:{:04x} 0x{:02x} 0x{:02x} {}",
            dev.bus,
            dev.slot,
            dev.func,
            dev.vendor_id,
            dev.device_id,
            dev.class_code,
            dev.subclass,
            dev.class_name()
        );
        for (i, bar) in dev.bars.iter().enumerate() {
            match bar {
                crate::pci::Bar::Memory { base_addr, size, .. } => {
                    kprintln!("   BAR{}: Memory at 0x{:08x} ({} KiB)", i, base_addr, size / 1024);
                }
                crate::pci::Bar::Io { port, size } => {
                    kprintln!("   BAR{}: I/O Port at 0x{:04x} ({} B)", i, port, size);
                }
                crate::pci::Bar::None => {}
            }
        }
    }
}

/// Handler for `net`: Displays network device status, MAC address, and tests frame transmission/reception.
fn cmd_net(args: &str) {
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Network Interface (Intel E1000) ===");
    vga::set_color(Color::White, Color::Black);

    let mut guard = crate::net::E1000_DEVICE.lock();
    let dev = match guard.as_mut() {
        Some(d) => d,
        None => {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("No active network controller detected on PCI bus.");
            vga::set_color(Color::White, Color::Black);
            return;
        }
    };

    let mac = dev.mac_addr;
    let link_up = dev.is_link_up();

    kprintln!(
        "  MAC Address:  {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    kprintln!("  Link Status:  {}", if link_up { "UP (1000 Mbps Full Duplex)" } else { "DOWN" });
    kprintln!("  BAR0 MMIO:    0x{:08x} (size: {} KiB)", dev.bar0_phys, dev.bar0_size / 1024);
    kprintln!("  DMA RX Ring:  0x{:08x} (8 descriptors, 2048B buffers)", dev.rx_ring_phys);
    kprintln!("  DMA TX Ring:  0x{:08x} (8 descriptors, 2048B buffers)", dev.tx_ring_phys);
    kprintln!("  Packets TX:   {} ({} bytes)", dev.tx_packet_count, dev.tx_bytes_count);
    kprintln!("  Packets RX:   {} ({} bytes)", dev.rx_packet_count, dev.rx_bytes_count);

    if args == "test" || args == "send" {
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("\n[NET] Transmitting test Ethernet frame (Broadcast ARP)...");
        vga::set_color(Color::White, Color::Black);

        // Construct standard 60-byte broadcast ARP request packet:
        let mut test_frame = [0u8; 60];
        test_frame[0..6].copy_from_slice(&[0xFF; 6]);
        test_frame[6..12].copy_from_slice(&mac);
        test_frame[12] = 0x08;
        test_frame[13] = 0x06;
        test_frame[14] = 0x00;
        test_frame[15] = 0x01;
        test_frame[16] = 0x08;
        test_frame[17] = 0x00;
        test_frame[18] = 0x06;
        test_frame[19] = 0x04;
        test_frame[20] = 0x00;
        test_frame[21] = 0x01;
        test_frame[22..28].copy_from_slice(&mac);
        test_frame[28..32].copy_from_slice(&[10, 0, 2, 15]);
        test_frame[32..38].copy_from_slice(&[0x00; 6]);
        test_frame[38..42].copy_from_slice(&[10, 0, 2, 2]);

        match dev.transmit(&test_frame) {
            Ok(()) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[NET] Packet transmitted successfully via DMA (DD completed).");
                vga::set_color(Color::White, Color::Black);

                // Check for incoming reply (e.g. from QEMU virtual router)
                let mut rx_buf = [0u8; 1518];
                let mut received = false;
                for _ in 0..20_000_000 {
                    if let Some(len) = dev.receive(&mut rx_buf) {
                        vga::set_color(Color::LightGreen, Color::Black);
                        kprintln!("[NET] Received incoming Ethernet frame: {} bytes!", len);
                        vga::set_color(Color::White, Color::Black);
                        let eth_type = ((rx_buf[12] as u16) << 8) | (rx_buf[13] as u16);
                        kprintln!(
                            "      From: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} | EtherType: 0x{:04x}",
                            rx_buf[6], rx_buf[7], rx_buf[8], rx_buf[9], rx_buf[10], rx_buf[11], eth_type
                        );
                        received = true;
                        break;
                    }
                    core::hint::spin_loop();
                }

                if !received {
                    // Test internal hardware MAC loopback verification
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[NET] Verifying hardware RX engine via MAC loopback mode...");
                    vga::set_color(Color::White, Color::Black);

                    unsafe {
                        let rctl = dev.mmio.read32(crate::net::e1000::REG_RCTL);
                        // Enable MAC loopback (LBM = 01b, bit 6)
                        dev.mmio.write32(crate::net::e1000::REG_RCTL, rctl | (1 << 6));
                    }

                    let mut loop_frame = [0u8; 64];
                    loop_frame[0..6].copy_from_slice(&mac);
                    loop_frame[6..12].copy_from_slice(&mac);
                    loop_frame[12] = 0x88;
                    loop_frame[13] = 0xB5; // IEEE 802 Local Experimental
                    loop_frame[14..28].copy_from_slice(b"E1000-LOOPBACK");

                    if let Ok(()) = dev.transmit(&loop_frame) {
                        for _ in 0..500_000 {
                            if let Some(len) = dev.receive(&mut rx_buf) {
                                vga::set_color(Color::LightGreen, Color::Black);
                                kprintln!("[NET] Hardware RX verification SUCCESS: Received {} byte loopback frame!", len);
                                vga::set_color(Color::White, Color::Black);
                                received = true;
                                break;
                            }
                            core::hint::spin_loop();
                        }
                    }

                    // Restore normal wire mode (clear LBM)
                    unsafe {
                        let rctl = dev.mmio.read32(crate::net::e1000::REG_RCTL);
                        dev.mmio.write32(crate::net::e1000::REG_RCTL, rctl & !(3 << 6));
                    }
                }

                if !received {
                    vga::set_color(Color::Yellow, Color::Black);
                    kprintln!("[NET] No reply received within polling window.");
                    vga::set_color(Color::White, Color::Black);
                }
            }
            Err(e) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("[NET] Transmission failed: {}", e);
                vga::set_color(Color::White, Color::Black);
            }
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
