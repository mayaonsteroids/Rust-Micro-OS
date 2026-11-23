//! System identity configuration and management.
//!
//! Provides centralized identity constants (configured username, default hostname,
//! OS information, architecture, and version) and runtime hostname configuration.

use spin::Mutex;

/// Central configured username for the BubbleOS environment.
pub const CONFIGURED_USERNAME: &str = "maya";

/// Default system hostname.
pub const DEFAULT_HOSTNAME: &str = "micro-os";

/// Official operating system name.
pub const OS_NAME: &str = "BubbleOS";

/// Operating system short description.
pub const OS_DESCRIPTION: &str = "A little OS with big possibilities.";

/// Kernel version.
pub const OS_VERSION: &str = "0.1.0";

/// Hardware architecture.
pub const ARCHITECTURE: &str = "x86_64";

/// CPU execution mode.
pub const CPU_MODE: &str = "Long Mode";

/// Current development milestone.
pub const MILESTONE: &str = "17";

/// Maximum length for a hostname string.
pub const MAX_HOSTNAME_LEN: usize = 63;

/// In-memory storage for runtime hostname.
struct HostnameStorage {
    bytes: [u8; MAX_HOSTNAME_LEN],
    len: usize,
}

impl HostnameStorage {
    const fn new() -> Self {
        let mut bytes = [0u8; MAX_HOSTNAME_LEN];
        let default_bytes = DEFAULT_HOSTNAME.as_bytes();
        let mut i = 0;
        while i < default_bytes.len() && i < MAX_HOSTNAME_LEN {
            bytes[i] = default_bytes[i];
            i += 1;
        }
        Self {
            bytes,
            len: default_bytes.len(),
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or(DEFAULT_HOSTNAME)
    }

    fn set(&mut self, new_name: &str) -> Result<(), &'static str> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Hostname cannot be empty");
        }
        if trimmed.len() > MAX_HOSTNAME_LEN {
            return Err("Hostname exceeds maximum length of 63 characters");
        }
        for b in trimmed.bytes() {
            if !b.is_ascii_alphanumeric() && b != b'-' && b != b'_' && b != b'.' {
                return Err("Hostname contains invalid characters (allowed: alphanumeric, '-', '_', '.')");
            }
        }
        self.bytes[..trimmed.len()].copy_from_slice(trimmed.as_bytes());
        self.len = trimmed.len();
        Ok(())
    }
}

static HOSTNAME: Mutex<HostnameStorage> = Mutex::new(HostnameStorage::new());

/// Returns the central configured username.
#[inline]
pub fn get_username() -> &'static str {
    CONFIGURED_USERNAME
}

/// Executes a closure with a borrowed view of the current hostname.
pub fn with_hostname<F, R>(f: F) -> R
where
    F: FnOnce(&str) -> R,
{
    let guard = HOSTNAME.lock();
    f(guard.as_str())
}

/// Sets the runtime hostname.
///
/// Note: This is an in-memory runtime configuration only and will safely reset
/// to `DEFAULT_HOSTNAME` upon reboot.
pub fn set_hostname(new_name: &str) -> Result<(), &'static str> {
    let mut guard = HOSTNAME.lock();
    guard.set(new_name)
}

/// Verified system information for uname and system display.
#[derive(Debug, Clone, Copy)]
pub struct SystemInfo {
    pub os_name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub architecture: &'static str,
    pub cpu_mode: &'static str,
    pub milestone: &'static str,
}

/// Returns verified system information.
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        os_name: OS_NAME,
        description: OS_DESCRIPTION,
        version: OS_VERSION,
        architecture: ARCHITECTURE,
        cpu_mode: CPU_MODE,
        milestone: MILESTONE,
    }
}
