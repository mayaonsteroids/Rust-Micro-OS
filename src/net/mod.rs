//! Network subsystem for `rust-micro-os`.
//!
//! Provides network device driver integration (Intel E1000) and packet transmission / reception.

pub mod e1000;

pub use e1000::{E1000Device, E1000_DEVICE};

/// Initializes the network subsystem by probing and configuring network hardware.
pub fn init() -> Result<(), &'static str> {
    e1000::init()
}

/// Transmits a raw Ethernet frame via the active network controller.
pub fn send_packet(packet: &[u8]) -> Result<(), &'static str> {
    let mut guard = E1000_DEVICE.lock();
    let dev = guard.as_mut().ok_or("Network device not initialized")?;
    dev.transmit(packet)
}

/// Polls for an incoming Ethernet frame into `buf`.
pub fn receive_packet(buf: &mut [u8]) -> Option<usize> {
    let mut guard = E1000_DEVICE.lock();
    let dev = guard.as_mut()?;
    dev.receive(buf)
}

/// Retrieves the hardware MAC address of the active network controller.
pub fn get_mac_address() -> Option<[u8; 6]> {
    let guard = E1000_DEVICE.lock();
    let dev = guard.as_ref()?;
    Some(dev.mac_addr)
}

/// Retrieves network transmission and reception statistics:
/// (tx_packets, tx_bytes, rx_packets, rx_bytes)
pub fn get_statistics() -> Option<(u64, u64, u64, u64)> {
    let guard = E1000_DEVICE.lock();
    let dev = guard.as_ref()?;
    Some((
        dev.tx_packet_count,
        dev.tx_bytes_count,
        dev.rx_packet_count,
        dev.rx_bytes_count,
    ))
}
