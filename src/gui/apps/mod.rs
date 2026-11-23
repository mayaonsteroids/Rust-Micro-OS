//! Desktop Applications Module.

pub mod about;
pub mod browser;
pub mod files;
pub mod music;
pub mod network;
pub mod settings;
pub mod sysmon;
pub mod terminal;

pub use about::AboutApp;
pub use browser::BrowserApp;
pub use files::FilesApp;
pub use music::MusicApp;
pub use network::NetworkApp;
pub use settings::SettingsApp;
pub use sysmon::SysMonApp;
pub use terminal::TerminalApp;
