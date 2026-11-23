//! Low-level PCI Configuration Space access via I/O ports 0xCF8 (CONFIG_ADDRESS)
//! and 0xCFC (CONFIG_DATA).

use x86_64::instructions::port::Port;

pub const PCI_CONFIG_ADDRESS: u16 = 0x0CF8;
pub const PCI_CONFIG_DATA: u16 = 0x0CFC;

/// Computes the 32-bit PCI configuration address value for the given bus, slot, func, and offset.
#[inline(always)]
fn pci_config_address(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    (1u32 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

/// Reads a 32-bit dword from PCI configuration space.
pub unsafe fn pci_config_read_u32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let address = pci_config_address(bus, slot, func, offset);
    let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

    addr_port.write(address);
    data_port.read()
}

/// Writes a 32-bit dword to PCI configuration space.
pub unsafe fn pci_config_write_u32(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    let address = pci_config_address(bus, slot, func, offset);
    let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

    addr_port.write(address);
    data_port.write(value);
}

/// Reads a 16-bit word from PCI configuration space.
pub unsafe fn pci_config_read_u16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let dword = pci_config_read_u32(bus, slot, func, offset);
    let shift = (offset & 2) * 8;
    ((dword >> shift) & 0xFFFF) as u16
}

/// Writes a 16-bit word to PCI configuration space.
pub unsafe fn pci_config_write_u16(bus: u8, slot: u8, func: u8, offset: u8, value: u16) {
    let dword = pci_config_read_u32(bus, slot, func, offset);
    let shift = (offset & 2) * 8;
    let mask = !(0xFFFFu32 << shift);
    let new_dword = (dword & mask) | ((value as u32) << shift);
    pci_config_write_u32(bus, slot, func, offset, new_dword);
}

/// Reads an 8-bit byte from PCI configuration space.
pub unsafe fn pci_config_read_u8(bus: u8, slot: u8, func: u8, offset: u8) -> u8 {
    let dword = pci_config_read_u32(bus, slot, func, offset);
    let shift = (offset & 3) * 8;
    ((dword >> shift) & 0xFF) as u8
}

/// Writes an 8-bit byte to PCI configuration space.
pub unsafe fn pci_config_write_u8(bus: u8, slot: u8, func: u8, offset: u8, value: u8) {
    let dword = pci_config_read_u32(bus, slot, func, offset);
    let shift = (offset & 3) * 8;
    let mask = !(0xFFu32 << shift);
    let new_dword = (dword & mask) | ((value as u32) << shift);
    pci_config_write_u32(bus, slot, func, offset, new_dword);
}
