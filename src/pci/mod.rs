//! PCI (Peripheral Component Interconnect) bus enumeration and configuration subsystem.

pub mod config;
pub mod device;

pub use config::*;
pub use device::*;

use alloc::vec::Vec;
use crate::info;

/// Global Intel E1000 IDs
pub const INTEL_VENDOR_ID: u16 = 0x8086;
pub const E1000_82540EM_DEVICE_ID: u16 = 0x100E;

/// Scans PCI buses 0..256 for all active PCI devices.
pub fn scan_pci() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    // Scan standard host buses (typically bus 0 in QEMU / standard PCs)
    for bus in 0..=32 {
        for slot in 0..32 {
            // Check function 0 first
            if let Some(dev0) = PciDevice::probe(bus, slot, 0) {
                let is_multi_function = (dev0.header_type & 0x80) != 0;
                devices.push(dev0);

                if is_multi_function {
                    for func in 1..8 {
                        if let Some(dev_func) = PciDevice::probe(bus, slot, func) {
                            devices.push(dev_func);
                        }
                    }
                }
            }
        }
    }

    devices
}

/// Finds a specific PCI device by vendor and device ID.
pub fn find_device(vendor_id: u16, device_id: u16) -> Option<PciDevice> {
    for bus in 0..=32 {
        for slot in 0..32 {
            if let Some(dev0) = PciDevice::probe(bus, slot, 0) {
                if dev0.vendor_id == vendor_id && dev0.device_id == device_id {
                    return Some(dev0);
                }
                let is_multi_function = (dev0.header_type & 0x80) != 0;
                if is_multi_function {
                    for func in 1..8 {
                        if let Some(dev_func) = PciDevice::probe(bus, slot, func) {
                            if dev_func.vendor_id == vendor_id && dev_func.device_id == device_id {
                                return Some(dev_func);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Finds a PCI device matching class and subclass code (e.g. 0x04:0x01 for Audio).
pub fn find_device_by_class(class_code: u8, subclass: u8) -> Option<PciDevice> {
    for bus in 0..=32 {
        for slot in 0..32 {
            if let Some(dev0) = PciDevice::probe(bus, slot, 0) {
                if dev0.class_code == class_code && dev0.subclass == subclass {
                    return Some(dev0);
                }
                let is_multi_function = (dev0.header_type & 0x80) != 0;
                if is_multi_function {
                    for func in 1..8 {
                        if let Some(dev_func) = PciDevice::probe(bus, slot, func) {
                            if dev_func.class_code == class_code && dev_func.subclass == subclass {
                                return Some(dev_func);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Initializes the PCI subsystem and logs discovered devices.
pub fn init() -> Vec<PciDevice> {
    info!("========================================================");
    info!("             SCANNING PCI CONFIGURATION SPACE           ");
    info!("========================================================");

    let devices = scan_pci();
    info!("[PCI] Enumeration complete: {} active device(s) found.", devices.len());

    for dev in &devices {
        info!(
            "[PCI] {:02x}:{:02x}.{} Vendor: 0x{:04x} Device: 0x{:04x} Class: 0x{:02x}:{:02x} ({})",
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
                Bar::Memory { base_addr, size, prefetchable, is_64bit } => {
                    info!(
                        "      BAR{}: Memory{} at 0x{:08x} (size: {} KiB, prefetch: {})",
                        i,
                        if *is_64bit { "64" } else { "32" },
                        base_addr,
                        size / 1024,
                        prefetchable
                    );
                }
                Bar::Io { port, size } => {
                    info!("      BAR{}: I/O Port at 0x{:04x} (size: {} bytes)", i, port, size);
                }
                Bar::None => {}
            }
        }
    }

    devices
}
