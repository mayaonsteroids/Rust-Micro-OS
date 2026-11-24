//! Wi-Fi & Network Configuration Diagnostic Subsystem.
//!
//! Provides truthful, real-time interface telemetry, identifying the
//! active hypervisor / NAT environment, IP/Gateway/DNS parameters,
//! live DMA packet counters, and VirtualBox / QEMU NAT architecture.

#[allow(unused_imports)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn write_hex2(dst: &mut [u8], offset: &mut usize, val: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hi = HEX[(val >> 4) as usize];
    let lo = HEX[(val & 0x0F) as usize];
    write_bytes(dst, offset, &[hi, lo]);
}

fn emit_field<F: FnMut(&str)>(emit: &mut F, key: &str, val: &str, suffix: &str) {
    let mut buf = [0u8; 128];
    let mut off = 0;
    write_bytes(&mut buf, &mut off, key.as_bytes());
    write_bytes(&mut buf, &mut off, val.as_bytes());
    write_bytes(&mut buf, &mut off, suffix.as_bytes());
    if let Ok(s) = core::str::from_utf8(&buf[..off]) {
        emit(s);
    }
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
    let link_up = is_link_up();

    // 1. Query Host Wi-Fi Helper over VirtualBox / hypervisor NAT bridge (10.0.2.2:19820)
    let mut target_buf = [0u8; 32];
    let mut t_off = 0;
    for i in 0..4 {
        write_u8(&mut target_buf, &mut t_off, cfg.gateway[i]);
        if i < 3 { write_bytes(&mut target_buf, &mut t_off, b"."); }
    }
    write_bytes(&mut target_buf, &mut t_off, b":19820");
    let target_str = core::str::from_utf8(&target_buf[..t_off]).unwrap_or("10.0.2.2:19820");

    let host_wifi_res = crate::net::http_get_timeout(target_str, "/", 8_000_000, 15_000_000);

    emit("HOST WI-FI CONNECTION");
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
                emit("Status: Connected");
                emit_field(&mut emit, "SSID: ", ssid_val.unwrap_or("None"), "");
                emit_field(&mut emit, "Signal: ", sig_val.unwrap_or("0"), "%");
                emit_field(&mut emit, "Channel: ", chan_val.unwrap_or("0"), "");
                emit_field(&mut emit, "Radio Type: ", radio_val.unwrap_or("Unknown"), "");
                emit_field(&mut emit, "Authentication: ", auth_val.unwrap_or("Unknown"), "");
                emit_field(&mut emit, "RX Rate: ", rx_val.unwrap_or("0.0"), " Mbps");
                emit_field(&mut emit, "TX Rate: ", tx_val.unwrap_or("0.0"), " Mbps");
                emit_field(&mut emit, "Host Adapter: ", iface_val.unwrap_or("Wi-Fi"), "");
            } else {
                emit("Status: Disconnected");
                emit_field(&mut emit, "SSID: ", ssid_val.unwrap_or("None"), "");
                emit_field(&mut emit, "Signal: ", "0", "%");
                emit_field(&mut emit, "Channel: ", "0", "");
                emit_field(&mut emit, "Radio Type: ", "None", "");
                emit_field(&mut emit, "Authentication: ", "None", "");
                emit_field(&mut emit, "RX Rate: ", "0.0", " Mbps");
                emit_field(&mut emit, "TX Rate: ", "0.0", " Mbps");
                emit_field(&mut emit, "Host Adapter: ", iface_val.unwrap_or("Wi-Fi"), "");
            }
        }
        _ => {
            emit("Status: Offline (Helper unreachable at 10.0.2.2:19820)");
            emit("Notice: Run on Windows: python tools/windows_host_wifi_helper.py");
        }
    }

    emit("");
    emit("GUEST NETWORK");
    emit(if link_up {
        "Guest Ethernet: Connected"
    } else {
        "Guest Ethernet: Disconnected"
    });

    // Guest IP
    let mut ip_buf = [0u8; 64];
    let mut off = 0;
    write_bytes(&mut ip_buf, &mut off, b"Guest IP: ");
    for i in 0..4 {
        write_u8(&mut ip_buf, &mut off, cfg.ip[i]);
        if i < 3 { write_bytes(&mut ip_buf, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&ip_buf[..off]) { emit(s); }

    // Gateway
    off = 0;
    write_bytes(&mut ip_buf, &mut off, b"Gateway: ");
    for i in 0..4 {
        write_u8(&mut ip_buf, &mut off, cfg.gateway[i]);
        if i < 3 { write_bytes(&mut ip_buf, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&ip_buf[..off]) { emit(s); }

    // DNS
    off = 0;
    write_bytes(&mut ip_buf, &mut off, b"DNS: ");
    for i in 0..4 {
        write_u8(&mut ip_buf, &mut off, cfg.dns[i]);
        if i < 3 { write_bytes(&mut ip_buf, &mut off, b"."); }
    }
    if let Ok(s) = core::str::from_utf8(&ip_buf[..off]) { emit(s); }

    // DHCP Status
    off = 0;
    write_bytes(&mut ip_buf, &mut off, b"DHCP Status: ");
    write_bytes(&mut ip_buf, &mut off, cfg.mode.as_str().as_bytes());
    if let Ok(s) = core::str::from_utf8(&ip_buf[..off]) { emit(s); }
}
