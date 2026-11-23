//! Intel 82540EM (E1000) Gigabit Ethernet Controller Driver.
//!
//! Conforms to Intel 8254x Family Gigabit Ethernet Controller Developer's Manual.
//! Provides MMIO register access, hardware initialization, MAC address discovery,
//! DMA descriptor rings, and packet transmission/reception.

use core::sync::atomic::{compiler_fence, Ordering};
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};
use crate::pci::device::PciDevice;
use crate::{info, warn};

// --- E1000 Register Offsets ---
pub const REG_CTRL: u32 = 0x00000;    // Device Control
pub const REG_STATUS: u32 = 0x00008;  // Device Status
pub const REG_EECD: u32 = 0x00010;    // EEPROM Control
pub const REG_EERD: u32 = 0x00014;    // EEPROM Read
pub const REG_ICR: u32 = 0x000C0;     // Interrupt Cause Read
pub const REG_IMS: u32 = 0x000D0;     // Interrupt Mask Set
pub const REG_IMC: u32 = 0x000D8;     // Interrupt Mask Clear
pub const REG_RCTL: u32 = 0x00100;    // Receive Control
pub const REG_TCTL: u32 = 0x00400;    // Transmit Control
pub const REG_TIPG: u32 = 0x00410;    // Transmit Inter-Packet Gap
pub const REG_RDBAL: u32 = 0x02800;   // RX Descriptor Base Address Low
pub const REG_RDBAH: u32 = 0x02804;   // RX Descriptor Base Address High
pub const REG_RDLEN: u32 = 0x02808;   // RX Descriptor Length
pub const REG_RDH: u32 = 0x02810;     // RX Descriptor Head
pub const REG_RDT: u32 = 0x02818;     // RX Descriptor Tail
pub const REG_TDBAL: u32 = 0x03800;   // TX Descriptor Base Address Low
pub const REG_TDBAH: u32 = 0x03804;   // TX Descriptor Base Address High
pub const REG_TDLEN: u32 = 0x03808;   // TX Descriptor Length
pub const REG_TDH: u32 = 0x03810;     // TX Descriptor Head
pub const REG_TDT: u32 = 0x03818;     // TX Descriptor Tail
pub const REG_MTA: u32 = 0x05200;     // Multicast Table Array (128 entries * 4 bytes)
pub const REG_RAL0: u32 = 0x05400;    // Receive Address Low 0
pub const REG_RAH0: u32 = 0x05404;    // Receive Address High 0

// --- Register Bit Flags ---
pub const CTRL_FD: u32 = 1 << 0;          // Full Duplex
pub const CTRL_ASDE: u32 = 1 << 5;        // Auto-Speed Detection Enable
pub const CTRL_SLU: u32 = 1 << 6;         // Set Link Up
pub const CTRL_SPEED_1000: u32 = 2 << 8;  // 1000 Mb/s
pub const CTRL_RST: u32 = 1 << 26;        // Software Reset

pub const STATUS_FD: u32 = 1 << 0;        // Full Duplex
pub const STATUS_LU: u32 = 1 << 1;        // Link Up

pub const RCTL_EN: u32 = 1 << 1;          // Receiver Enable
pub const RCTL_SBP: u32 = 1 << 2;         // Store Bad Packets
pub const RCTL_UPE: u32 = 1 << 3;         // Unicast Promiscuous Enable
pub const RCTL_MPE: u32 = 1 << 4;         // Multicast Promiscuous Enable
pub const RCTL_LPE: u32 = 1 << 5;         // Long Packet Enable
pub const RCTL_BAM: u32 = 1 << 15;        // Broadcast Accept Mode
pub const RCTL_BSIZE_2048: u32 = 0 << 16; // 2048-byte buffer size
pub const RCTL_SECRC: u32 = 1 << 26;      // Strip Ethernet CRC

pub const TCTL_EN: u32 = 1 << 1;          // Transmit Enable
pub const TCTL_PSP: u32 = 1 << 3;         // Pad Short Packets
pub const TCTL_CT_VAL: u32 = 0x0F << 4;   // Collision Threshold (15)
pub const TCTL_COLD_VAL: u32 = 0x40 << 12;// Collision Distance (64 for full-duplex)

// --- DMA Descriptor Constants ---
pub const NUM_RX_DESCRIPTORS: usize = 8;
pub const NUM_TX_DESCRIPTORS: usize = 8;
pub const PACKET_BUFFER_SIZE: usize = 2048;

/// E1000 Legacy Receive Descriptor (16 bytes).
#[repr(C, packed)]
pub struct RxDescriptor {
    pub buffer_addr: u64, // Physical DMA buffer address
    pub length: u16,      // Length of received packet
    pub checksum: u16,    // Checksum
    pub status: u8,       // Status (Bit 0 = DD: Descriptor Done, Bit 1 = EOP)
    pub errors: u8,       // Errors
    pub special: u16,     // Special
}

/// E1000 Legacy Transmit Descriptor (16 bytes).
#[repr(C, packed)]
pub struct TxDescriptor {
    pub buffer_addr: u64, // Physical DMA buffer address
    pub length: u16,      // Data buffer length
    pub cso: u8,          // Checksum offset
    pub cmd: u8,          // Command (Bit 0 = EOP, Bit 1 = IFCS, Bit 3 = RS)
    pub status: u8,       // Status (Bit 0 = DD: Descriptor Done)
    pub css: u8,          // Checksum start
    pub special: u16,     // Special
}

/// Hardware MMIO register accessor.
pub struct E1000Mmio {
    base_virt: VirtAddr,
}

impl E1000Mmio {
    pub const fn new(base_virt: VirtAddr) -> Self {
        Self { base_virt }
    }

    #[inline(always)]
    pub unsafe fn read32(&self, offset: u32) -> u32 {
        let ptr = (self.base_virt.as_u64() + offset as u64) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    #[inline(always)]
    pub unsafe fn write32(&self, offset: u32, val: u32) {
        let ptr = (self.base_virt.as_u64() + offset as u64) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }
}

/// Active Intel E1000 Network Device controller.
pub struct E1000Device {
    pub mmio: E1000Mmio,
    pub bar0_phys: u64,
    pub bar0_size: usize,
    pub mac_addr: [u8; 6],
    pub rx_ring_virt: *mut RxDescriptor,
    pub rx_ring_phys: u64,
    pub tx_ring_virt: *mut TxDescriptor,
    pub tx_ring_phys: u64,
    pub rx_buffers_virt: [*mut u8; NUM_RX_DESCRIPTORS],
    pub tx_buffers_virt: [*mut u8; NUM_TX_DESCRIPTORS],
    pub rx_cur: usize,
    pub tx_tail: usize,
    pub rx_packet_count: u64,
    pub tx_packet_count: u64,
    pub rx_bytes_count: u64,
    pub tx_bytes_count: u64,
}

unsafe impl Send for E1000Device {}
unsafe impl Sync for E1000Device {}

impl E1000Device {
    /// Initializes the E1000 hardware device.
    pub fn new(mut pci_dev: PciDevice) -> Result<Self, &'static str> {
        let (bar0_phys, bar0_size) = pci_dev
            .get_bar0_memory()
            .ok_or("E1000 BAR0 is not a valid Memory BAR")?;

        info!("[E1000] Detected BAR0 at physical 0x{:08x} (size: {} KiB)", bar0_phys, bar0_size / 1024);

        // Enable Bus Mastering (DMA) and Memory Space access on the PCI device
        pci_dev.enable_memory_space();
        pci_dev.enable_bus_mastering();
        let cmd = pci_dev.read_command();
        info!("[E1000] PCI Command register configured: 0x{:04x} (Bus Master: ON, Memory Space: ON)", cmd);

        // Map BAR0 into kernel virtual memory
        let bar0_virt = crate::memory::map_mmio_range(PhysAddr::new(bar0_phys), bar0_size);
        let mmio = E1000Mmio::new(bar0_virt);

        // 1. Disable / Mask all interrupts (we poll safely)
        unsafe {
            mmio.write32(REG_IMC, 0xFFFF_FFFF);
            let _ = mmio.read32(REG_ICR);
        }

        // 2. Perform software reset (RST bit 26 in CTRL)
        unsafe {
            let ctrl = mmio.read32(REG_CTRL);
            mmio.write32(REG_CTRL, ctrl | CTRL_RST);
        }

        // Wait for reset to complete (hardware clears RST bit)
        let mut reset_done = false;
        for _ in 0..100_000 {
            compiler_fence(Ordering::SeqCst);
            let ctrl = unsafe { mmio.read32(REG_CTRL) };
            if (ctrl & CTRL_RST) == 0 {
                reset_done = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !reset_done {
            warn!("[E1000] Software reset timeout; continuing...");
        }

        // Disable interrupts again following reset
        unsafe {
            mmio.write32(REG_IMC, 0xFFFF_FFFF);
            let _ = mmio.read32(REG_ICR);
        }

        // 3. Set Link Up (SLU), Auto-Speed, 1000 Mbps, Full Duplex
        unsafe {
            let ctrl = mmio.read32(REG_CTRL);
            mmio.write32(REG_CTRL, ctrl | CTRL_SLU | CTRL_ASDE | CTRL_SPEED_1000 | CTRL_FD);
        }

        // 4. Clear Multicast Table Array (MTA)
        unsafe {
            for i in 0..128 {
                mmio.write32(REG_MTA + (i * 4), 0);
            }
        }

        // 5. Read MAC address from hardware registers RAL0 / RAH0 and program Address Valid
        let mut mac_addr = [0u8; 6];
        unsafe {
            let ral = mmio.read32(REG_RAL0);
            let rah = mmio.read32(REG_RAH0);

            mac_addr[0] = (ral & 0xFF) as u8;
            mac_addr[1] = ((ral >> 8) & 0xFF) as u8;
            mac_addr[2] = ((ral >> 16) & 0xFF) as u8;
            mac_addr[3] = ((ral >> 24) & 0xFF) as u8;
            mac_addr[4] = (rah & 0xFF) as u8;
            mac_addr[5] = ((rah >> 8) & 0xFF) as u8;

            // Ensure RAL0 and RAH0 are explicitly programmed with AV (bit 31 = Address Valid)
            let ral_val = (mac_addr[0] as u32)
                | ((mac_addr[1] as u32) << 8)
                | ((mac_addr[2] as u32) << 16)
                | ((mac_addr[3] as u32) << 24);
            let rah_val = (mac_addr[4] as u32)
                | ((mac_addr[5] as u32) << 8)
                | (1u32 << 31);
            mmio.write32(REG_RAL0, ral_val);
            mmio.write32(REG_RAH0, rah_val);
        }

        info!(
            "[E1000] Hardware MAC Address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac_addr[0], mac_addr[1], mac_addr[2], mac_addr[3], mac_addr[4], mac_addr[5]
        );

        // 6. Allocate DMA memory frames:
        // Frame 1: Descriptor Rings (RX at offset 0, TX at offset 2048)
        let ring_frame = crate::memory::address_space::allocate_frame()
            .ok_or("Failed to allocate physical frame for E1000 descriptor rings")?;
        let ring_phys = ring_frame.start_address().as_u64();
        let ring_virt = crate::memory::address_space::phys_to_virt(ring_frame.start_address());

        let rx_ring_phys = ring_phys;
        let rx_ring_virt = ring_virt.as_mut_ptr::<RxDescriptor>();

        let tx_ring_phys = ring_phys + 2048;
        let tx_ring_virt = (ring_virt + 2048u64).as_mut_ptr::<TxDescriptor>();

        // Zero descriptor rings
        unsafe {
            core::ptr::write_bytes(rx_ring_virt as *mut u8, 0, NUM_RX_DESCRIPTORS * core::mem::size_of::<RxDescriptor>());
            core::ptr::write_bytes(tx_ring_virt as *mut u8, 0, NUM_TX_DESCRIPTORS * core::mem::size_of::<TxDescriptor>());
        }

        // Allocate 4 physical frames for 8 RX packet buffers (2048 bytes each)
        let mut rx_buffers_virt = [core::ptr::null_mut(); NUM_RX_DESCRIPTORS];
        for i in 0..(NUM_RX_DESCRIPTORS / 2) {
            let frame = crate::memory::address_space::allocate_frame()
                .ok_or("Failed to allocate frame for RX packet buffers")?;
            let f_phys = frame.start_address().as_u64();
            let f_virt = crate::memory::address_space::phys_to_virt(frame.start_address());

            let idx0 = i * 2;
            let idx1 = i * 2 + 1;

            rx_buffers_virt[idx0] = f_virt.as_mut_ptr();
            rx_buffers_virt[idx1] = (f_virt + 2048u64).as_mut_ptr();

            unsafe {
                let desc0 = &mut *rx_ring_virt.add(idx0);
                desc0.buffer_addr = f_phys;
                desc0.status = 0;

                let desc1 = &mut *rx_ring_virt.add(idx1);
                desc1.buffer_addr = f_phys + 2048;
                desc1.status = 0;
            }
        }

        // Allocate 4 physical frames for 8 TX packet buffers (2048 bytes each)
        let mut tx_buffers_virt = [core::ptr::null_mut(); NUM_TX_DESCRIPTORS];
        for i in 0..(NUM_TX_DESCRIPTORS / 2) {
            let frame = crate::memory::address_space::allocate_frame()
                .ok_or("Failed to allocate frame for TX packet buffers")?;
            let f_phys = frame.start_address().as_u64();
            let f_virt = crate::memory::address_space::phys_to_virt(frame.start_address());

            let idx0 = i * 2;
            let idx1 = i * 2 + 1;

            tx_buffers_virt[idx0] = f_virt.as_mut_ptr();
            tx_buffers_virt[idx1] = (f_virt + 2048u64).as_mut_ptr();

            unsafe {
                let desc0 = &mut *tx_ring_virt.add(idx0);
                desc0.buffer_addr = f_phys;
                desc0.cmd = 0;
                desc0.status = 0;

                let desc1 = &mut *tx_ring_virt.add(idx1);
                desc1.buffer_addr = f_phys + 2048;
                desc1.cmd = 0;
                desc1.status = 0;
            }
        }

        info!(
            "[E1000] DMA Rings: RX at 0x{:08x}, TX at 0x{:08x} (Allocated 9 frames = 36 KiB)",
            rx_ring_phys, tx_ring_phys
        );

        // 7. Configure RX Hardware Registers
        unsafe {
            mmio.write32(REG_RDBAL, (rx_ring_phys & 0xFFFF_FFFF) as u32);
            mmio.write32(REG_RDBAH, ((rx_ring_phys >> 32) & 0xFFFF_FFFF) as u32);
            mmio.write32(REG_RDLEN, (NUM_RX_DESCRIPTORS * core::mem::size_of::<RxDescriptor>()) as u32);
            mmio.write32(REG_RDH, 0);
            mmio.write32(REG_RDT, (NUM_RX_DESCRIPTORS - 1) as u32);

            // RCTL: Enable, Broadcast Accept, Multicast Promiscuous, Unicast Promiscuous, 2048-byte buffer, Strip CRC
            let rctl = RCTL_EN | RCTL_BAM | RCTL_MPE | RCTL_UPE | RCTL_BSIZE_2048 | RCTL_SECRC;
            mmio.write32(REG_RCTL, rctl);
        }

        // 8. Configure TX Hardware Registers
        unsafe {
            mmio.write32(REG_TDBAL, (tx_ring_phys & 0xFFFF_FFFF) as u32);
            mmio.write32(REG_TDBAH, ((tx_ring_phys >> 32) & 0xFFFF_FFFF) as u32);
            mmio.write32(REG_TDLEN, (NUM_TX_DESCRIPTORS * core::mem::size_of::<TxDescriptor>()) as u32);
            mmio.write32(REG_TDH, 0);
            mmio.write32(REG_TDT, 0);

            // TIPG: standard recommended IPG for full-duplex Gigabit (0x0060200A)
            mmio.write32(REG_TIPG, 0x0060200A);

            // TCTL: Enable, Pad Short Packets, CT=15, COLD=64
            let tctl = TCTL_EN | TCTL_PSP | TCTL_CT_VAL | TCTL_COLD_VAL;
            mmio.write32(REG_TCTL, tctl);
        }

        info!("[E1000] Hardware initialization complete: RX and TX engines enabled.");

        Ok(Self {
            mmio,
            bar0_phys,
            bar0_size,
            mac_addr,
            rx_ring_virt,
            rx_ring_phys,
            tx_ring_virt,
            tx_ring_phys,
            rx_buffers_virt,
            tx_buffers_virt,
            rx_cur: 0,
            tx_tail: 0,
            rx_packet_count: 0,
            tx_packet_count: 0,
            rx_bytes_count: 0,
            tx_bytes_count: 0,
        })
    }

    /// Transmits an Ethernet packet frame via DMA.
    ///
    /// Sets EOP (End of Packet), IFCS (Insert FCS/CRC), and RS (Report Status).
    /// Polls descriptor status for DD (Descriptor Done) bit with timeout.
    pub fn transmit(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if packet.is_empty() || packet.len() > PACKET_BUFFER_SIZE {
            return Err("Packet length exceeds maximum buffer capacity (2048 bytes)");
        }

        let cur = self.tx_tail;
        let desc = unsafe { &mut *self.tx_ring_virt.add(cur) };

        // Copy packet data to virtual DMA buffer
        unsafe {
            let buf_ptr = self.tx_buffers_virt[cur];
            core::ptr::copy_nonoverlapping(packet.as_ptr(), buf_ptr, packet.len());
        }

        // Configure transmit descriptor:
        // CMD: EOP (0x01) | IFCS (0x02) | RS (0x08) = 0x0B
        desc.length = packet.len() as u16;
        desc.cso = 0;
        desc.css = 0;
        desc.special = 0;
        desc.status = 0; // Clear DD bit before queueing
        desc.cmd = 0x01 | 0x02 | 0x08;

        compiler_fence(Ordering::SeqCst);

        // Advance Transmit Descriptor Tail (TDT) to trigger hardware DMA fetch
        self.tx_tail = (self.tx_tail + 1) % NUM_TX_DESCRIPTORS;
        unsafe {
            self.mmio.write32(REG_TDT, self.tx_tail as u32);
        }

        // Poll descriptor DD bit for completion with bounded timeout
        let mut completed = false;
        for _ in 0..100_000 {
            compiler_fence(Ordering::SeqCst);
            let status = unsafe { core::ptr::read_volatile(&desc.status) };
            if (status & 0x01) != 0 {
                completed = true;
                break;
            }
            core::hint::spin_loop();
        }

        if completed {
            self.tx_packet_count += 1;
            self.tx_bytes_count += packet.len() as u64;
            Ok(())
        } else {
            Err("Transmission timeout waiting for Descriptor Done (DD)")
        }
    }

    /// Polls the receive descriptor ring for an incoming Ethernet packet frame.
    ///
    /// If a packet has arrived (DD bit is set), copies packet data into `buf`
    /// and resets the descriptor, advancing RDT to return the buffer to hardware.
    pub fn receive(&mut self, buf: &mut [u8]) -> Option<usize> {
        let cur = self.rx_cur;
        let desc = unsafe { &mut *self.rx_ring_virt.add(cur) };

        compiler_fence(Ordering::SeqCst);
        let status = unsafe { core::ptr::read_volatile(&desc.status) };

        // Check if Descriptor Done (DD) is set by hardware
        if (status & 0x01) == 0 {
            return None;
        }

        let len = desc.length as usize;
        let copy_len = core::cmp::min(len, buf.len());

        unsafe {
            let src_ptr = self.rx_buffers_virt[cur];
            core::ptr::copy_nonoverlapping(src_ptr, buf.as_mut_ptr(), copy_len);
        }

        // Reset descriptor state for future receptions
        desc.status = 0;
        desc.length = 0;
        compiler_fence(Ordering::SeqCst);

        // Advance RDT to return descriptor to hardware
        let old_cur = self.rx_cur;
        self.rx_cur = (self.rx_cur + 1) % NUM_RX_DESCRIPTORS;
        unsafe {
            self.mmio.write32(REG_RDT, old_cur as u32);
        }

        self.rx_packet_count += 1;
        self.rx_bytes_count += len as u64;
        Some(copy_len)
    }

    /// Returns hardware link status: true if link is up.
    pub fn is_link_up(&self) -> bool {
        let status = unsafe { self.mmio.read32(REG_STATUS) };
        (status & STATUS_LU) != 0
    }
}

/// Global Intel E1000 controller instance.
pub static E1000_DEVICE: Mutex<Option<E1000Device>> = Mutex::new(None);

/// Probes PCI for Intel E1000 (Vendor 0x8086, Device 0x100E) and initializes the driver.
pub fn init() -> Result<(), &'static str> {
    info!("========================================================");
    info!("             INITIALIZING INTEL E1000 DRIVER            ");
    info!("========================================================");

    let dev = match crate::pci::find_device(crate::pci::INTEL_VENDOR_ID, crate::pci::E1000_82540EM_DEVICE_ID) {
        Some(d) => d,
        None => {
            info!("[E1000] No Intel E1000 network adapter detected on PCI bus.");
            return Err("E1000 device not found");
        }
    };

    info!(
        "[E1000] Found 82540EM controller at PCI {:02x}:{:02x}.{}",
        dev.bus, dev.slot, dev.func
    );

    let e1000 = E1000Device::new(dev)?;
    let mac = e1000.mac_addr;
    let link_up = e1000.is_link_up();

    info!(
        "[E1000] Driver ready! MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} | Link: {}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5],
        if link_up { "UP (1000 Mbps)" } else { "DOWN" }
    );

    *E1000_DEVICE.lock() = Some(e1000);
    Ok(())
}
