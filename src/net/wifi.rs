//! Wi-Fi & Network Configuration Diagnostic Subsystem.
//!
//! Provides truthful, real-time interface telemetry, identifying the
//! active hypervisor / NAT environment, IP/Gateway/DNS parameters,
//! live DMA packet counters, and VirtualBox / QEMU NAT architecture.

use crate::net::{get_config, get_mac_address, get_statistics, is_device_present, is_link_up};

fn write_bytes(dst: &mut [u8], offset: &mut usize, src: &[u8]) {
    for &b in src {
        if *offset < dst.len() {
            dst[*offset] = b;
            *offset += 1;
        }
    }
}

fn write_u8(dst: &mut [u8], offset: &mut usize, mut val: u8) {
    if val == 0 {
        write_bytes(dst, offset, b"0");
        return;
    }
    let mut temp = [0u8; 3];
    let mut len = 0;
    while val > 0 {
        temp[len] = b'0' + (val % 10);
        val /= 10;
        len += 1;
    }
    for i in 0..len {
        write_bytes(dst, offset, &[temp[len - 1 - i]]);
    }
}

fn write_u64(dst: &mut [u8], offset: &mut usize, mut val: u64) {
    if val == 0 {
        write_bytes(dst, offset, b"0");
        return;
    }
    let mut temp = [0u8; 20];
    let mut len = 0;
    while val > 0 {
        temp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    for i in 0..len {
        write_bytes(dst, offset, &[temp[len - 1 - i]]);
    }
}

fn write_hex2(dst: &mut [u8], offset: &mut usize, val: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hi = HEX[(val >> 4) as usize];
    let lo = HEX[(val & 0x0F) as usize];
    write_bytes(dst, offset, &[hi, lo]);
}

fn extract_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_idx = json.find(key)?;
    let after_key = &json[key_idx + key.len()..];
    let colon_idx = after_key.find(':')?;
    let after_colon = &after_key[colon_idx + 1..];
    let quote1 = after_colon.find('"')?;
    let val_start = &after_colon[quote1 + 1..];
    let quote2 = val_start.find('"')?;
    Some(&val_start[..quote2])
}

fn extract_json_raw_val<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_idx = json.find(key)?;
    let after_key = &json[key_idx + key.len()..];
    let colon_idx = after_key.find(':')?;
    let after_colon = after_key[colon_idx + 1..].trim_start();
    let mut end = 0;
    for b in after_colon.bytes() {
        if b == b',' || b == b'}' || b == b'\r' || b == b'\n' {
            break;
        }
        end += 1;
    }
    if end > 0 {
        Some(after_colon[..end].trim())
    } else {
        None
    }
}

/// Formats and emits full real-time network configuration diagnostics to a callback.
pub fn format_wifi_diagnostics<F: FnMut(&str)>(mut emit: F) {
    let cfg = get_config();
    let mac = get_mac_address().unwrap_or(cfg.mac);
    let dev_present = is_device_present();
    let link_up = is_link_up();
    let stats = get_statistics();

    // 1. Query Host Wi-Fi Helper over VirtualBox / hypervisor NAT bridge (10.0.2.2:19820)
    let mut target_buf = [0u8; 32];
    let mut t_off = 0;
    for i in 0..4 {
        write_u8(&mut target_buf, &mut t_off, cfg.gateway[i]);
        if i < 3 { write_bytes(&mut target_buf, &mut t_off, b"."); }
    }
    write_bytes(&mut target_buf, &mut t_off, b":19820");
    let target_str = core::str::from_utf8(&target_buf[..t_off]).unwrap_or("10.0.2.2:19820");

    let host_wifi_res = crate::net::http_get_timeout(target_str, "/", 1_500_000, 2_000_000);

    emit("=== HOST WI-FI CONNECTION (WINDOWS BRIDGE) ===");
    match host_wifi_res {
        Ok(ref resp) if resp.status_code == 200 => {
            let body = resp.body_str();
            let status_val = extract_json_str(body, "\"status\"");
            let ssid_val = extract_json_str(body, "\"ssid\"");
            let sig_val = extract_json_raw_val(body, "\"signal\"");
            let chan_val = extract_json_raw_val(body, "\"channel\"");
            let radio_val = extract_json_str(body, "\"radio_type\"");
            let auth_val = extract_json_str(body, "\"authentication\"");
            let rx_val = extract_json_raw_val(body, "\"rx_rate_mbps\"");
            let tx_val = extract_json_raw_val(body, "\"tx_rate_mbps\"");
            let iface_val = extract_json_str(body, "\"interface\"");

            let is_connected = status_val == Some("connected");
            if is_connected {
                emit("  Host Wi-Fi Status: Connected (Host WLAN Active)");
                if let Some(ssid) = ssid_val {
                    let mut line = [0u8; 96];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  SSID:              ");
                    write_bytes(&mut line, &mut off, ssid.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if let Some(sig) = sig_val {
                    let mut line = [0u8; 64];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Signal Strength:   ");
                    write_bytes(&mut line, &mut off, sig.as_bytes());
                    write_bytes(&mut line, &mut off, b"%");
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if let Some(chan) = chan_val {
                    let mut line = [0u8; 64];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Channel:           ");
                    write_bytes(&mut line, &mut off, chan.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if let Some(radio) = radio_val {
                    let mut line = [0u8; 64];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Radio Type:        ");
                    write_bytes(&mut line, &mut off, radio.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if let Some(auth) = auth_val {
                    let mut line = [0u8; 64];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Authentication:    ");
                    write_bytes(&mut line, &mut off, auth.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if rx_val.is_some() || tx_val.is_some() {
                    let mut line = [0u8; 96];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Link Rates:        Rx: ");
                    write_bytes(&mut line, &mut off, rx_val.unwrap_or("0").as_bytes());
                    write_bytes(&mut line, &mut off, b" Mbps | Tx: ");
                    write_bytes(&mut line, &mut off, tx_val.unwrap_or("0").as_bytes());
                    write_bytes(&mut line, &mut off, b" Mbps");
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
                if let Some(iface) = iface_val {
                    let mut line = [0u8; 96];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Host Interface:    ");
                    write_bytes(&mut line, &mut off, iface.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
            } else {
                emit("  Host Wi-Fi Status: Disconnected (No Active Host Wi-Fi)");
                emit("  SSID:              None");
                if let Some(iface) = iface_val {
                    let mut line = [0u8; 96];
                    let mut off = 0;
                    write_bytes(&mut line, &mut off, b"  Host Interface:    ");
                    write_bytes(&mut line, &mut off, iface.as_bytes());
                    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }
                }
            }
        }
        _ => {
            emit("  Host Wi-Fi Status: Offline (Host Wi-Fi Helper not responding on 10.0.2.2:19820)");
            emit("  Notice: Start helper on Windows host for live Wi-Fi telemetry:");
            emit("          python tools/windows_host_wifi_helper.py");
        }
    }

    emit("--------------------------------------------------");
    emit("=== GUEST NETWORK ADAPTER (INTEL 82540EM) ===");

    // Detect virtualized hypervisor environment from MAC address vendor OUI
    let is_vbox = mac[0] == 0x08 && mac[1] == 0x00 && mac[2] == 0x27;
    let is_qemu = mac[0] == 0x52 && mac[1] == 0x54 && mac[2] == 0x00;

    let env_str = if is_vbox {
        "Oracle VirtualBox (NAT Virtual Ethernet)"
    } else if is_qemu {
        "QEMU Hypervisor (SLIRP User Networking)"
    } else if dev_present {
        "Standard PCI Ethernet (Intel 82540EM Gigabit)"
    } else {
        "No Ethernet Hardware Detected"
    };

    let status_str = if !dev_present {
        "DEVICE NOT DETECTED"
    } else if link_up {
        if is_vbox {
            "CONNECTED (VirtualBox NAT Active)"
        } else if is_qemu {
            "CONNECTED (QEMU SLIRP Active)"
        } else {
            "CONNECTED (Gigabit Link Up)"
        }
    } else {
        "DISCONNECTED (Link Down)"
    };
    
    // Status line
    let mut line = [0u8; 96];
    let mut off = 0;
    write_bytes(&mut line, &mut off, b"Connection Status: ");
    write_bytes(&mut line, &mut off, status_str.as_bytes());
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Environment
    off = 0;
    write_bytes(&mut line, &mut off, b"Environment:       ");
    write_bytes(&mut line, &mut off, env_str.as_bytes());
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Interface Name
    if dev_present {
        emit("Network Interface: eth0 (Intel 82540EM 1000 Mbps Full Duplex)");
    } else {
        emit("Network Interface: None (PCI 0x8086:0x100E not present)");
    }

    // IPv4 Address
    off = 0;
    write_bytes(&mut line, &mut off, b"IPv4 Address:      ");
    for i in 0..4 {
        write_u8(&mut line, &mut off, cfg.ip[i]);
        if i < 3 { write_bytes(&mut line, &mut off, b"."); }
    }
    write_bytes(&mut line, &mut off, b" / 24");
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Subnet Mask
    off = 0;
    write_bytes(&mut line, &mut off, b"Subnet Mask:       ");
    for i in 0..4 {
        write_u8(&mut line, &mut off, cfg.subnet_mask[i]);
        if i < 3 { write_bytes(&mut line, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Default Gateway
    off = 0;
    write_bytes(&mut line, &mut off, b"Default Gateway:   ");
    for i in 0..4 {
        write_u8(&mut line, &mut off, cfg.gateway[i]);
        if i < 3 { write_bytes(&mut line, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // DNS Server
    off = 0;
    write_bytes(&mut line, &mut off, b"DNS Server:        ");
    for i in 0..4 {
        write_u8(&mut line, &mut off, cfg.dns[i]);
        if i < 3 { write_bytes(&mut line, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // MAC Address
    off = 0;
    write_bytes(&mut line, &mut off, b"MAC Address:       ");
    for i in 0..6 {
        write_hex2(&mut line, &mut off, mac[i]);
        if i < 5 { write_bytes(&mut line, &mut off, b":"); }
    }
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Real Traffic Counters (No fake .max() or fallback constants)
    off = 0;
    write_bytes(&mut line, &mut off, b"DMA Traffic:       ");
    if let Some((tx_p, tx_b, rx_p, rx_b)) = stats {
        write_bytes(&mut line, &mut off, b"Rx: ");
        write_u64(&mut line, &mut off, rx_p);
        write_bytes(&mut line, &mut off, b"p (");
        write_u64(&mut line, &mut off, rx_b);
        write_bytes(&mut line, &mut off, b"B) | Tx: ");
        write_u64(&mut line, &mut off, tx_p);
        write_bytes(&mut line, &mut off, b"p (");
        write_u64(&mut line, &mut off, tx_b);
        write_bytes(&mut line, &mut off, b"B)");
    } else {
        write_bytes(&mut line, &mut off, b"Unavailable (Controller Offline)");
    }
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    // Configuration Mode from real NetworkConfig
    off = 0;
    write_bytes(&mut line, &mut off, b"Configuration:     ");
    write_bytes(&mut line, &mut off, cfg.mode.as_str().as_bytes());
    if let Ok(s) = core::str::from_utf8(&line[..off]) { emit(s); }

    emit("--------------------------------------------------");
    emit("Architecture:      VirtualBox / QEMU Virtual Ethernet NAT Bridge");
    emit("Physical Uplink:   Host Computer Network (Wi-Fi / Ethernet)");
    emit("Notice: BubbleOS operates via an emulated Intel 82540EM Ethernet adapter.");
    emit("Hypervisor NAT automatically routes packets through your host connection.");
    emit("Guest does not possess native physical Wi-Fi hardware; host Wi-Fi is shared.");
}
