//! Text-based Web Browser subsystem for `rust-micro-os`.
//!
//! Provides single-tab terminal browsing, HTML-to-text rendering,
//! hyperlink navigation, and back-history tracking.

pub mod html;
pub mod browser;

pub use html::{
    ParsedPage, PageLink, ParsedUrl, parse_html, parse_url, resolve_relative_url,
    MAX_RENDERED_TEXT_LEN, MAX_TITLE_LEN, MAX_LINKS, MAX_URL_LEN,
};
pub use browser::{BrowserState, HistoryStack, run_browser};
