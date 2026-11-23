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

    // Ensure CPU interrupts are enabled for keyboard IRQ1 and timer delivery
    x86_64::instructions::interrupts::enable();

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
        "http" => cmd_http(args),
        "browser" => cmd_browser(args),
        "list" => cmd_list(),
        "tasks" => cmd_tasks(),
        "mem" => cmd_mem(),
        "trace" => cmd_trace(),
        "run" => cmd_run(args),
        "wifi" => cmd_wifi(),
        "desktop" | "gui" => cmd_desktop(),
        "audio" | "music" => cmd_audio(args),
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
    kprintln!("  net [cmd]     - Network stack: info, arp, dhcp, dns, ping, udp, tcp, test");
    kprintln!("  http get <h>  - Fetch web content via HTTP/1.1 (e.g. 'http get example.com /')");
    kprintln!("  browser [url] - Launch single-tab text web browser");
    kprintln!("  list          - List available Ring 3 user programs");
    kprintln!("  tasks         - Display active and registered kernel tasks and processes");
    kprintln!("  mem           - Display physical frame allocation and free-list statistics");
    kprintln!("  trace         - Dump diagnostics circular trace buffer");
    kprintln!("  run <program> - Execute a user program in isolated Ring 3 (hello, counter, calc)");
    kprintln!("  wifi          - Display network configuration & Wi-Fi diagnostics");
    kprintln!("  desktop       - Launch Light Lavender windowed desktop GUI (or 'gui')");
    kprintln!("  audio [cmd]   - Control audio engine (play, pause, stop, next, prev, status)");
}

/// Handler for `wifi`: Displays live network configuration and Wi-Fi bridge diagnostics.
fn cmd_wifi() {
    crate::net::wifi::format_wifi_diagnostics(|line| {
        if line.starts_with("===") {
            vga::set_color(Color::LightCyan, Color::Black);
            kprintln!("{}", line);
        } else if line.starts_with("Connection Status") || line.starts_with("Notice") {
            vga::set_color(Color::LightGreen, Color::Black);
            kprintln!("{}", line);
        } else if line.starts_with("Host Wi-Fi") {
            vga::set_color(Color::LightRed, Color::Black);
            kprintln!("{}", line);
        } else {
            vga::set_color(Color::White, Color::Black);
            kprintln!("{}", line);
        }
    });
    vga::set_color(Color::White, Color::Black);
}

/// Handler for `clear`: Clears the VGA text screen and serial console.
fn cmd_clear() {
    vga::init_lavender_palette();
    vga::clear_screen();
    crate::serial_print!("\x1b[2J\x1b[H");
}

/// Handler for `desktop` / `gui`: Launches the interactive desktop environment.
fn cmd_desktop() {
    crate::gui::run_desktop();
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

/// Parses an IPv4 address from string format "A.B.C.D".
fn parse_ipv4_address(s: &str) -> Option<[u8; 4]> {
    let mut parts = s.split('.');
    let a = parse_u8(parts.next()?)?;
    let b = parse_u8(parts.next()?)?;
    let c = parse_u8(parts.next()?)?;
    let d = parse_u8(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}

fn parse_u8(s: &str) -> Option<u8> {
    let mut val: u16 = 0;
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u16;
    }
    if val <= 255 {
        Some(val as u8)
    } else {
        None
    }
}

fn parse_u16(s: &str) -> Option<u16> {
    let mut val: u32 = 0;
    if s.is_empty() || s.len() > 5 {
        return None;
    }
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u32;
    }
    if val <= 65535 {
        Some(val as u16)
    } else {
        None
    }
}

/// Handler for `net`: Displays network device status, MAC address, ARP table, and diagnostics.
fn cmd_net(args: &str) {
    let dev_info = {
        let guard = crate::net::E1000_DEVICE.lock();
        match guard.as_ref() {
            Some(dev) => Some((
                dev.mac_addr,
                dev.is_link_up(),
                dev.bar0_phys,
                dev.bar0_size,
                dev.rx_ring_phys,
                dev.tx_ring_phys,
                dev.tx_packet_count,
                dev.tx_bytes_count,
                dev.rx_packet_count,
                dev.rx_bytes_count,
            )),
            None => None,
        }
    };

    let (mac, link_up, bar0_phys, bar0_size, rx_ring, tx_ring, tx_pkts, tx_bytes, rx_pkts, rx_bytes) =
        match dev_info {
            Some(info) => info,
            None => {
                vga::set_color(Color::Yellow, Color::Black);
                kprintln!("No active network controller detected on PCI bus.");
                vga::set_color(Color::White, Color::Black);
                return;
            }
        };

    let subcmd = args.trim();

    if subcmd == "info" {
        let cfg = crate::net::get_config();
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("=== Network Interface Configuration ===");
        vga::set_color(Color::White, Color::Black);
        kprintln!("  Interface:    eth0 (Intel 82540EM Gigabit)");
        kprintln!(
            "  MAC Address:  {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            cfg.mac[0], cfg.mac[1], cfg.mac[2], cfg.mac[3], cfg.mac[4], cfg.mac[5]
        );
        kprintln!(
            "  IPv4 Address: {}.{}.{}.{}",
            cfg.ip[0], cfg.ip[1], cfg.ip[2], cfg.ip[3]
        );
        kprintln!(
            "  Subnet Mask:  {}.{}.{}.{}",
            cfg.subnet_mask[0], cfg.subnet_mask[1], cfg.subnet_mask[2], cfg.subnet_mask[3]
        );
        kprintln!(
            "  Gateway:      {}.{}.{}.{}",
            cfg.gateway[0], cfg.gateway[1], cfg.gateway[2], cfg.gateway[3]
        );
        kprintln!(
            "  DNS Server:   {}.{}.{}.{}",
            cfg.dns[0], cfg.dns[1], cfg.dns[2], cfg.dns[3]
        );
        kprintln!("  Config Mode:  {}", cfg.mode.as_str());
        if let Some(lease_secs) = cfg.lease_time_secs {
            kprintln!("  DHCP Lease:   {} seconds", lease_secs);
        }
        return;
    }

    if subcmd == "dhcp" {
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("[DHCP] Broadcasting DHCPDISCOVER on local network...");
        vga::set_color(Color::White, Color::Black);

        match crate::net::acquire_dhcp_lease(20_000_000) {
            Ok(lease) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[DHCP] DHCP lease acquired successfully!");
                vga::set_color(Color::White, Color::Black);
                kprintln!(
                    "  Assigned IP:  {}.{}.{}.{}",
                    lease.assigned_ip[0], lease.assigned_ip[1], lease.assigned_ip[2], lease.assigned_ip[3]
                );
                kprintln!(
                    "  Subnet Mask:  {}.{}.{}.{}",
                    lease.subnet_mask[0], lease.subnet_mask[1], lease.subnet_mask[2], lease.subnet_mask[3]
                );
                kprintln!(
                    "  Gateway:      {}.{}.{}.{}",
                    lease.gateway[0], lease.gateway[1], lease.gateway[2], lease.gateway[3]
                );
                kprintln!(
                    "  DNS Server:   {}.{}.{}.{}",
                    lease.dns_server[0], lease.dns_server[1], lease.dns_server[2], lease.dns_server[3]
                );
                kprintln!("  Lease Time:   {} seconds", lease.lease_time_secs);
                kprintln!(
                    "  DHCP Server:  {}.{}.{}.{}",
                    lease.server_ip[0], lease.server_ip[1], lease.server_ip[2], lease.server_ip[3]
                );
            }
            Err(e) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("[DHCP] Lease acquisition failed: {}", e);
                vga::set_color(Color::White, Color::Black);
            }
        }
        return;
    }

    if subcmd.starts_with("dns") {
        let domain = subcmd["dns".len()..].trim();
        if domain.is_empty() {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("Usage: net dns <domain>");
            kprintln!("Example: net dns example.com");
            vga::set_color(Color::White, Color::Black);
            return;
        }

        let cfg = crate::net::get_config();
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!(
            "[DNS] Querying A-record for '{}' via {}.{}.{}.{}:53...",
            domain, cfg.dns[0], cfg.dns[1], cfg.dns[2], cfg.dns[3]
        );
        vga::set_color(Color::White, Color::Black);

        match crate::net::resolve_dns(domain, 20_000_000) {
            Ok(resolved_ip) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!(
                    "[DNS] '{}' resolved to {}.{}.{}.{}",
                    domain, resolved_ip[0], resolved_ip[1], resolved_ip[2], resolved_ip[3]
                );
                vga::set_color(Color::White, Color::Black);
            }
            Err(e) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("[DNS] Resolution failed: {}", e);
                vga::set_color(Color::White, Color::Black);
            }
        }
        return;
    }

    if subcmd == "arp" {
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("=== ARP Cache Table ===");
        vga::set_color(Color::White, Color::Black);
        kprintln!("IP ADDRESS       MAC ADDRESS        STATUS");
        kprintln!("------------------------------------------------");
        let cache = crate::net::ARP_CACHE.lock();
        let mut count = 0;
        for entry in cache.entries.iter() {
            if entry.valid {
                kprintln!(
                    "{:3}.{:3}.{:3}.{:3}  {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  DYNAMIC (VALID)",
                    entry.ip[0], entry.ip[1], entry.ip[2], entry.ip[3],
                    entry.mac[0], entry.mac[1], entry.mac[2], entry.mac[3], entry.mac[4], entry.mac[5]
                );
                count += 1;
            }
        }
        if count == 0 {
            kprintln!("  (ARP cache is currently empty. Run 'net test' or 'net ping' to resolve)");
        }
        return;
    }

    if subcmd.starts_with("ping") {
        let parts: alloc::vec::Vec<&str> = subcmd.split_whitespace().collect();
        let target_ip = if parts.len() > 1 {
            match parse_ipv4_address(parts[1]) {
                Some(ip) => ip,
                None => {
                    vga::set_color(Color::LightRed, Color::Black);
                    kprintln!("Usage: net ping [IPv4 Address]");
                    vga::set_color(Color::White, Color::Black);
                    return;
                }
            }
        } else {
            crate::net::get_config().gateway
        };

        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!(
            "PING {}.{}.{}.{} with 64 bytes of ICMP data...",
            target_ip[0], target_ip[1], target_ip[2], target_ip[3]
        );
        vga::set_color(Color::White, Color::Black);

        match crate::net::ping(target_ip, 20_000_000) {
            Ok(_spins) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!(
                    "64 bytes from {}.{}.{}.{}: icmp_seq=1 ttl=64 verified (genuine ICMP reply)",
                    target_ip[0], target_ip[1], target_ip[2], target_ip[3]
                );
                vga::set_color(Color::White, Color::Black);
            }
            Err(e) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("Ping failed: {}", e);
                vga::set_color(Color::White, Color::Black);
            }
        }
        return;
    }

    if subcmd.starts_with("udp") {
        let parts: alloc::vec::Vec<&str> = subcmd.split_whitespace().collect();
        let target_ip = if parts.len() > 1 {
            parse_ipv4_address(parts[1]).unwrap_or([10, 0, 2, 2])
        } else {
            [10, 0, 2, 2]
        };
        let msg = if parts.len() > 2 {
            parts[2].as_bytes()
        } else {
            b"Hello from Rust Micro-OS UDP!"
        };

        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!(
            "[NET] Sending UDP datagram to {}.{}.{}.{}:5000 ({} bytes)...",
            target_ip[0], target_ip[1], target_ip[2], target_ip[3], msg.len()
        );
        vga::set_color(Color::White, Color::Black);

        match crate::net::send_udp(target_ip, 5000, 49152, msg) {
            Ok(()) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[NET] UDP datagram transmitted successfully via IPv4/Ethernet.");
                vga::set_color(Color::White, Color::Black);
            }
            Err(e) => {
                vga::set_color(Color::LightRed, Color::Black);
                kprintln!("[NET] UDP transmission failed: {}", e);
                vga::set_color(Color::White, Color::Black);
            }
        }
        return;
    }

    if subcmd.starts_with("tcp") {
        let tcp_args = subcmd["tcp".len()..].trim();
        if tcp_args.is_empty() || tcp_args == "help" {
            vga::set_color(Color::Yellow, Color::Black);
            kprintln!("Usage: net tcp test [ip] [port]");
            kprintln!("Example: net tcp test 10.0.2.2 80");
            vga::set_color(Color::White, Color::Black);
            return;
        }

        if tcp_args.starts_with("test") {
            let test_params = tcp_args["test".len()..].trim();
            let mut parts = test_params.split_whitespace();
            let target_ip = if let Some(ip_str) = parts.next() {
                parse_ipv4_address(ip_str).unwrap_or([10, 0, 2, 2])
            } else {
                [10, 0, 2, 2]
            };
            let target_port = if let Some(port_str) = parts.next() {
                parse_u16(port_str).unwrap_or(80)
            } else {
                80
            };

            vga::set_color(Color::LightCyan, Color::Black);
            kprintln!(
                "[TCP] Initiating 3-way handshake with {}.{}.{}.{}:{}...",
                target_ip[0], target_ip[1], target_ip[2], target_ip[3], target_port
            );
            vga::set_color(Color::White, Color::Black);

            match crate::net::tcp_connect(target_ip, target_port, 20_000_000) {
                Ok(mut stream) => {
                    vga::set_color(Color::LightGreen, Color::Black);
                    kprintln!("[TCP] Connection established! State: ESTABLISHED");
                    kprintln!(
                        "      Local Port: {} | Remote Seq: {} | Local Seq: {}",
                        stream.local_port, stream.remote_seq, stream.local_seq
                    );
                    vga::set_color(Color::White, Color::Black);

                    // Transmit test HTTP GET payload
                    let test_msg = b"GET / HTTP/1.1\r\nHost: 10.0.2.2\r\nConnection: close\r\n\r\n";
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[TCP] Transmitting {} bytes of HTTP test payload...", test_msg.len());
                    vga::set_color(Color::White, Color::Black);

                    if let Err(e) = stream.write(test_msg) {
                        vga::set_color(Color::LightRed, Color::Black);
                        kprintln!("[TCP] Payload transmission failed: {}", e);
                        vga::set_color(Color::White, Color::Black);
                    } else {
                        vga::set_color(Color::LightGreen, Color::Black);
                        kprintln!("[TCP] Payload transmitted (ACK confirmed).");
                        vga::set_color(Color::White, Color::Black);

                        // Read response
                        let mut rx_buf = [0u8; 1024];
                        match stream.read_to_end(&mut rx_buf, 20_000_000) {
                            Ok(n) => {
                                vga::set_color(Color::LightGreen, Color::Black);
                                kprintln!("[TCP] Received {} response bytes from peer.", n);
                                vga::set_color(Color::White, Color::Black);
                                if n > 0 {
                                    let preview_len = core::cmp::min(n, 120);
                                    if let Ok(s) = core::str::from_utf8(&rx_buf[..preview_len]) {
                                        kprintln!("      Data: \"{}\"...", s);
                                    }
                                }
                            }
                            Err(e) => {
                                vga::set_color(Color::Yellow, Color::Black);
                                kprintln!("[TCP] Read note: {}", e);
                                vga::set_color(Color::White, Color::Black);
                            }
                        }
                    }

                    // Teardown
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[TCP] Closing connection (FIN teardown)...");
                    vga::set_color(Color::White, Color::Black);
                    stream.close();
                    vga::set_color(Color::LightGreen, Color::Black);
                    kprintln!("[TCP] FIN segment transmitted cleanly.");
                    vga::set_color(Color::White, Color::Black);
                }
                Err(e) => {
                    vga::set_color(Color::LightRed, Color::Black);
                    kprintln!("[TCP] Handshake failed: {}", e);
                    vga::set_color(Color::White, Color::Black);
                }
            }
            return;
        }
    }

    // Default `net` output (interface hardware status)
    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("=== Network Interface (Intel E1000) ===");
    vga::set_color(Color::White, Color::Black);
    kprintln!(
        "  MAC Address:  {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    kprintln!("  Link Status:  {}", if link_up { "UP (1000 Mbps Full Duplex)" } else { "DOWN" });
    kprintln!("  BAR0 MMIO:    0x{:08x} (size: {} KiB)", bar0_phys, bar0_size / 1024);
    kprintln!("  DMA RX Ring:  0x{:08x} (8 descriptors, 2048B buffers)", rx_ring);
    kprintln!("  DMA TX Ring:  0x{:08x} (8 descriptors, 2048B buffers)", tx_ring);
    kprintln!("  Packets TX:   {} ({} bytes)", tx_pkts, tx_bytes);
    kprintln!("  Packets RX:   {} ({} bytes)", rx_pkts, rx_bytes);

    if subcmd == "test" || subcmd == "send" {
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("\n[NET] Transmitting test Ethernet frame (Broadcast ARP)...");
        vga::set_color(Color::White, Color::Black);

        // Construct standard 60-byte broadcast ARP request packet
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

        match crate::net::send_packet(&test_frame) {
            Ok(()) => {
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[NET] Packet transmitted successfully via DMA (DD completed).");
                vga::set_color(Color::White, Color::Black);

                // Check for incoming reply (e.g. from QEMU virtual router)
                let mut rx_buf = [0u8; 1518];
                let mut received = false;
                for _ in 0..20_000_000 {
                    if let Some(len) = crate::net::receive_packet(&mut rx_buf) {
                        vga::set_color(Color::LightGreen, Color::Black);
                        kprintln!("[NET] Received incoming Ethernet frame: {} bytes!", len);
                        vga::set_color(Color::White, Color::Black);
                        let eth_type = ((rx_buf[12] as u16) << 8) | (rx_buf[13] as u16);
                        kprintln!(
                            "      From: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} | EtherType: 0x{:04x}",
                            rx_buf[6], rx_buf[7], rx_buf[8], rx_buf[9], rx_buf[10], rx_buf[11], eth_type
                        );
                        // Also feed to stack
                        crate::net::process_incoming_frame(&rx_buf[..len]);
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

                    {
                        let mut guard = crate::net::E1000_DEVICE.lock();
                        if let Some(ref mut dev) = *guard {
                            unsafe {
                                let rctl = dev.mmio.read32(crate::net::e1000::REG_RCTL);
                                dev.mmio.write32(crate::net::e1000::REG_RCTL, rctl | (1 << 6));
                            }
                        }
                    }

                    let mut loop_frame = [0u8; 64];
                    loop_frame[0..6].copy_from_slice(&mac);
                    loop_frame[6..12].copy_from_slice(&mac);
                    loop_frame[12] = 0x88;
                    loop_frame[13] = 0xB5;
                    loop_frame[14..28].copy_from_slice(b"E1000-LOOPBACK");

                    if let Ok(()) = crate::net::send_packet(&loop_frame) {
                        for _ in 0..500_000 {
                            if let Some(len) = crate::net::receive_packet(&mut rx_buf) {
                                vga::set_color(Color::LightGreen, Color::Black);
                                kprintln!("[NET] Hardware RX verification SUCCESS: Received {} byte loopback frame!", len);
                                vga::set_color(Color::White, Color::Black);
                                received = true;
                                break;
                            }
                            core::hint::spin_loop();
                        }
                    }

                    {
                        let mut guard = crate::net::E1000_DEVICE.lock();
                        if let Some(ref mut dev) = *guard {
                            unsafe {
                                let rctl = dev.mmio.read32(crate::net::e1000::REG_RCTL);
                                dev.mmio.write32(crate::net::e1000::REG_RCTL, rctl & !(3 << 6));
                            }
                        }
                    }
                }

                if !received {
                    vga::set_color(Color::Yellow, Color::Black);
                    kprintln!("[NET] No reply received within polling window.");
                    vga::set_color(Color::White, Color::Black);
                } else {
                    // Dynamic ARP resolution test
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[NET] Testing dynamic ARP resolution for default gateway (10.0.2.2)...");
                    vga::set_color(Color::White, Color::Black);
                    match crate::net::resolve_arp([10, 0, 2, 2], 5_000_000) {
                        Ok(gw_mac) => {
                            vga::set_color(Color::LightGreen, Color::Black);
                            kprintln!(
                                "[NET] Gateway resolved: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                gw_mac[0], gw_mac[1], gw_mac[2], gw_mac[3], gw_mac[4], gw_mac[5]
                            );
                            vga::set_color(Color::White, Color::Black);
                        }
                        Err(e) => {
                            vga::set_color(Color::Yellow, Color::Black);
                            kprintln!("[NET] Gateway resolution note: {}", e);
                            vga::set_color(Color::White, Color::Black);
                        }
                    }

                    // Dynamic UDP transmission test
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[NET] Testing UDP datagram transmission to gateway (10.0.2.2:5000)...");
                    vga::set_color(Color::White, Color::Black);
                    match crate::net::send_udp([10, 0, 2, 2], 5000, 49152, b"MicroOS-UDP-Test") {
                        Ok(()) => {
                            vga::set_color(Color::LightGreen, Color::Black);
                            kprintln!("[NET] UDP datagram transmitted successfully via DMA.");
                            vga::set_color(Color::White, Color::Black);
                        }
                        Err(e) => {
                            vga::set_color(Color::Yellow, Color::Black);
                            kprintln!("[NET] UDP transmission note: {}", e);
                            vga::set_color(Color::White, Color::Black);
                        }
                    }

                    // Dynamic ICMP Ping test
                    vga::set_color(Color::LightCyan, Color::Black);
                    kprintln!("[NET] Testing ICMP Echo Ping to default gateway (10.0.2.2)...");
                    vga::set_color(Color::White, Color::Black);
                    match crate::net::ping([10, 0, 2, 2], 10_000_000) {
                        Ok(_spins) => {
                            vga::set_color(Color::LightGreen, Color::Black);
                            kprintln!("[NET] Ping reply from 10.0.2.2 verified! (genuine ICMP Echo Reply)");
                            vga::set_color(Color::White, Color::Black);
                        }
                        Err(e) => {
                            vga::set_color(Color::Yellow, Color::Black);
                            kprintln!("[NET] ICMP ping note: {}", e);
                            vga::set_color(Color::White, Color::Black);
                        }
                    }
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

/// Handler for `http`: Performs HTTP/1.1 GET request to specified hostname and path.
fn cmd_http(args: &str) {
    let args = args.trim();
    if args.is_empty() {
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("Usage: http get <hostname> [path]");
        kprintln!("Example: http get example.com /");
        kprintln!("         http get 10.0.2.2:8080 /");
        vga::set_color(Color::White, Color::Black);
        return;
    }

    let target = if args.starts_with("get ") {
        args["get ".len()..].trim()
    } else if args.starts_with("GET ") {
        args["GET ".len()..].trim()
    } else {
        args
    };

    let mut parts = target.split_whitespace();
    let host_or_url = parts.next().unwrap_or("").trim();
    let explicit_path = parts.next().unwrap_or("").trim();

    // Strip http:// or https:// if provided
    let host_or_url = if let Some(stripped) = host_or_url.strip_prefix("http://") {
        stripped
    } else if let Some(stripped) = host_or_url.strip_prefix("https://") {
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("[HTTP] Note: HTTPS is not yet supported in M17.5. Falling back to HTTP.");
        vga::set_color(Color::White, Color::Black);
        stripped
    } else {
        host_or_url
    };

    let (hostname, path) = if !explicit_path.is_empty() {
        (host_or_url, explicit_path)
    } else if let Some(slash_idx) = host_or_url.find('/') {
        let (h, p) = host_or_url.split_at(slash_idx);
        (h, p)
    } else {
        (host_or_url, "/")
    };

    let clean_path = if path.starts_with('/') { path } else { "/" };

    vga::set_color(Color::LightCyan, Color::Black);
    kprintln!("[HTTP] Sending GET request to http://{}{}", hostname, clean_path);
    vga::set_color(Color::White, Color::Black);

    match crate::net::http_get(hostname, clean_path) {
        Ok(resp) => {
            vga::set_color(Color::LightGreen, Color::Black);
            kprintln!("[HTTP] Response Status: {}", resp.status_code);
            vga::set_color(Color::White, Color::Black);
            kprintln!(
                "  Content-Length: {} bytes (chunked: {}, truncated: {})",
                resp.content_length, resp.is_chunked, resp.is_truncated
            );
            kprintln!("  Body Received:  {} bytes", resp.body_len);
            if resp.body_len > 0 {
                vga::set_color(Color::LightCyan, Color::Black);
                kprintln!("--- Response Body Preview ---");
                vga::set_color(Color::White, Color::Black);
                let preview_len = core::cmp::min(resp.body_len, 400);
                if let Ok(body_str) = core::str::from_utf8(&resp.body[..preview_len]) {
                    kprintln!("{}", body_str);
                    if resp.body_len > 400 {
                        vga::set_color(Color::Yellow, Color::Black);
                        kprintln!("... [truncated remaining {} bytes]", resp.body_len - 400);
                        vga::set_color(Color::White, Color::Black);
                    }
                } else {
                    kprintln!("[Binary or non-UTF8 payload: {} bytes]", resp.body_len);
                }
            }
        }
        Err(e) => {
            vga::set_color(Color::LightRed, Color::Black);
            kprintln!("[HTTP] Request failed: {}", e);
            vga::set_color(Color::White, Color::Black);
        }
    }
}

/// Handler for `browser`: Launches single-tab text-mode web browser.
fn cmd_browser(args: &str) {
    let initial_url = if args.is_empty() {
        None
    } else {
        Some(args.trim())
    };
    crate::browser::run_browser(initial_url);
}

/// Handler for `audio` / `music`: Controls the audio engine and music playback.
fn cmd_audio(args: &str) {
    let subcmd = args.trim();
    if subcmd.is_empty() || subcmd.eq_ignore_ascii_case("status") {
        let audio = crate::audio::AUDIO_ENGINE.lock();
        let track = audio.current_track();
        let state_str = match audio.state {
            crate::audio::PlaybackState::Playing => "PLAYING",
            crate::audio::PlaybackState::Paused => "PAUSED",
            crate::audio::PlaybackState::Stopped => "STOPPED",
        };
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("=== Audio Subsystem Status ===");
        vga::set_color(Color::White, Color::Black);
        kprintln!("  Driver:   {}", audio.driver_name());
        kprintln!("  State:    [{}]", state_str);
        kprintln!("  Track:    {} - {}", track.title, track.artist);
        kprintln!("  Album:    {}", track.album);
        kprintln!("  Duration: {}s (Progress: {}%)", track.duration_seconds, audio.progress_percent());
        kprintln!("  Volume:   {}%", audio.volume);
        kprintln!("  Repeat:   {}", if audio.is_repeat { "Enabled" } else { "Disabled" });
        return;
    }

    if subcmd.eq_ignore_ascii_case("play") {
        crate::audio::AUDIO_ENGINE.lock().play();
        vga::set_color(Color::LightGreen, Color::Black);
        let audio = crate::audio::AUDIO_ENGINE.lock();
        let track = audio.current_track();
        kprintln!("[AUDIO] Playing: '{}' by {}", track.title, track.artist);
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.eq_ignore_ascii_case("pause") {
        crate::audio::AUDIO_ENGINE.lock().pause();
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("[AUDIO] Playback paused.");
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.eq_ignore_ascii_case("stop") {
        crate::audio::AUDIO_ENGINE.lock().stop();
        vga::set_color(Color::LightRed, Color::Black);
        kprintln!("[AUDIO] Playback stopped.");
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.eq_ignore_ascii_case("next") {
        crate::audio::AUDIO_ENGINE.lock().next_track();
        let audio = crate::audio::AUDIO_ENGINE.lock();
        let track = audio.current_track();
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("[AUDIO] Switched to next track: '{}'", track.title);
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.eq_ignore_ascii_case("prev") {
        crate::audio::AUDIO_ENGINE.lock().prev_track();
        let audio = crate::audio::AUDIO_ENGINE.lock();
        let track = audio.current_track();
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("[AUDIO] Switched to previous track: '{}'", track.title);
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.eq_ignore_ascii_case("repeat") {
        let is_rep = crate::audio::AUDIO_ENGINE.lock().toggle_repeat();
        vga::set_color(Color::LightCyan, Color::Black);
        kprintln!("[AUDIO] Repeat: {}", if is_rep { "ON" } else { "OFF" });
        vga::set_color(Color::White, Color::Black);
    } else if subcmd.starts_with("vol ") || subcmd.starts_with("volume ") {
        let parts: alloc::vec::Vec<&str> = subcmd.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(vol) = parts[1].parse::<u8>() {
                crate::audio::AUDIO_ENGINE.lock().set_volume(vol);
                vga::set_color(Color::LightGreen, Color::Black);
                kprintln!("[AUDIO] Volume set to {}%", vol.min(100));
                vga::set_color(Color::White, Color::Black);
                return;
            }
        }
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("Usage: audio vol <0-100>");
        vga::set_color(Color::White, Color::Black);
    } else {
        vga::set_color(Color::Yellow, Color::Black);
        kprintln!("Usage: audio [play | pause | stop | next | prev | repeat | vol <0-100> | status]");
        vga::set_color(Color::White, Color::Black);
    }
}

