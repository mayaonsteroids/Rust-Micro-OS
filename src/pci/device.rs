//! PCI Device representation and Base Address Register (BAR) decoding.

use crate::pci::config::*;

/// Decoded PCI Base Address Register (BAR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    None,
    Memory {
        base_addr: u64,
        size: usize,
        prefetchable: bool,
        is_64bit: bool,
    },
    Io {
        port: u16,
        size: usize,
    },
}

/// Discovered PCI device descriptor.
#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub header_type: u8,
    pub bars: [Bar; 6],
}

impl PciDevice {
    /// Probes and constructs a `PciDevice` at `(bus, slot, func)`.
    /// Returns `None` if the device is absent (vendor ID 0xFFFF).
    pub fn probe(bus: u8, slot: u8, func: u8) -> Option<Self> {
        unsafe {
            let vendor_id = pci_config_read_u16(bus, slot, func, 0x00);
            if vendor_id == 0xFFFF {
                return None;
            }

            let device_id = pci_config_read_u16(bus, slot, func, 0x02);
            let command = pci_config_read_u16(bus, slot, func, 0x04);
            let status = pci_config_read_u16(bus, slot, func, 0x06);
            let revision_id = pci_config_read_u8(bus, slot, func, 0x08);
            let prog_if = pci_config_read_u8(bus, slot, func, 0x09);
            let subclass = pci_config_read_u8(bus, slot, func, 0x0A);
            let class_code = pci_config_read_u8(bus, slot, func, 0x0B);
            let header_type = pci_config_read_u8(bus, slot, func, 0x0E);

            let mut dev = Self {
                bus,
                slot,
                func,
                vendor_id,
                device_id,
                command,
                status,
                revision_id,
                prog_if,
                subclass,
                class_code,
                header_type,
                bars: [Bar::None; 6],
            };

            // Only standard devices (header type 0) have 6 standard BARs
            if (header_type & 0x7F) == 0 {
                dev.decode_bars();
            }

            Some(dev)
        }
    }

    /// Decodes all BAR registers for header type 0 devices.
    unsafe fn decode_bars(&mut self) {
        let mut bar_idx = 0;
        while bar_idx < 6 {
            let offset = 0x10 + (bar_idx as u8) * 4;
            let bar_val = pci_config_read_u32(self.bus, self.slot, self.func, offset);

            if bar_val == 0 || bar_val == 0xFFFF_FFFF {
                self.bars[bar_idx] = Bar::None;
                bar_idx += 1;
                continue;
            }

            if (bar_val & 1) == 0 {
                // Memory BAR
                let bar_type = (bar_val >> 1) & 0x03;
                let is_64bit = bar_type == 2;
                let prefetchable = ((bar_val >> 3) & 1) == 1;

                // Determine memory region size
                pci_config_write_u32(self.bus, self.slot, self.func, offset, 0xFFFF_FFFF);
                let size_mask = pci_config_read_u32(self.bus, self.slot, self.func, offset);
                pci_config_write_u32(self.bus, self.slot, self.func, offset, bar_val); // Restore!

                let masked = size_mask & !0x0F;
                let size = if masked != 0 {
                    (!(masked) as usize).wrapping_add(1)
                } else {
                    0
                };

                let mut base_addr = (bar_val & !0x0F) as u64;

                if is_64bit && bar_idx + 1 < 6 {
                    let next_offset = offset + 4;
                    let high_val = pci_config_read_u32(self.bus, self.slot, self.func, next_offset);
                    base_addr |= (high_val as u64) << 32;

                    self.bars[bar_idx] = Bar::Memory {
                        base_addr,
                        size,
                        prefetchable,
                        is_64bit: true,
                    };
                    self.bars[bar_idx + 1] = Bar::None; // High dword absorbed
                    bar_idx += 2;
                } else {
                    self.bars[bar_idx] = Bar::Memory {
                        base_addr,
                        size,
                        prefetchable,
                        is_64bit: false,
                    };
                    bar_idx += 1;
                }
            } else {
                // I/O BAR
                let port = (bar_val & !0x03) as u16;

                pci_config_write_u32(self.bus, self.slot, self.func, offset, 0xFFFF_FFFF);
                let size_mask = pci_config_read_u32(self.bus, self.slot, self.func, offset);
                pci_config_write_u32(self.bus, self.slot, self.func, offset, bar_val); // Restore!

                let masked = (size_mask & !0x03) as u16;
                let size = if masked != 0 {
                    (!(masked) as usize).wrapping_add(1)
                } else {
                    0
                };

                self.bars[bar_idx] = Bar::Io { port, size };
                bar_idx += 1;
            }
        }
    }

    /// Reads the current command register from hardware.
    pub fn read_command(&self) -> u16 {
        unsafe { pci_config_read_u16(self.bus, self.slot, self.func, 0x04) }
    }

    /// Writes the command register to hardware.
    pub fn write_command(&mut self, cmd: u16) {
        unsafe { pci_config_write_u16(self.bus, self.slot, self.func, 0x04, cmd) };
        self.command = cmd;
    }

    /// Enables Memory Space decoding (bit 1 of PCI Command register).
    pub fn enable_memory_space(&mut self) {
        let cmd = self.read_command() | (1 << 1);
        self.write_command(cmd);
    }

    /// Enables Bus Mastering / DMA (bit 2 of PCI Command register).
    pub fn enable_bus_mastering(&mut self) {
        let cmd = self.read_command() | (1 << 2);
        self.write_command(cmd);
    }

    /// Enables I/O Space decoding (bit 0 of PCI Command register).
    pub fn enable_io_space(&mut self) {
        let cmd = self.read_command() | (1 << 0);
        self.write_command(cmd);
    }

    /// Retrieves BAR0 if it is a Memory BAR.
    pub fn get_bar0_memory(&self) -> Option<(u64, usize)> {
        match self.bars[0] {
            Bar::Memory { base_addr, size, .. } => Some((base_addr, size)),
            _ => None,
        }
    }

    /// Returns a human-readable class name string.
    pub fn class_name(&self) -> &'static str {
        match self.class_code {
            0x00 => "Legacy Device",
            0x01 => "Mass Storage Controller",
            0x02 => "Network Controller",
            0x03 => "Display Controller",
            0x04 => "Multimedia Controller",
            0x05 => "Memory Controller",
            0x06 => "Bridge Device",
            0x07 => "Simple Communications Controller",
            0x08 => "Base System Peripheral",
            0x09 => "Input Device Controller",
            0x0C => "Serial Bus Controller",
            _ => "Other Device",
        }
    }
}
