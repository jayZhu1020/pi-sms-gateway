//! ASCII line-ending bytes and the rules used by our AT decoder.
//!
//! Interpretation rules for this decoder (not every possible serial protocol):
//! - Text bytes are buffered; CR and LF never become part of the returned text.
//! - CR starts an ending: wait for LF, allowing extra CR bytes in between.
//! - LF completes the line, with or without a preceding CR; skip empty lines.
//! - Text after CR but before LF is invalid, rather than overwriting old text.
//! The CR flag persists across reads, so a split CRLF behaves like a joined one.

// These Rust escapes represent single ASCII control bytes, not printed characters.
// CR (carriage return, byte 13) historically moves a cursor to the line's start.
// LF (line feed, byte 10) historically moves it down one line.
// Here they mark AT line boundaries; we do not move a cursor or overwrite text.
pub const CR: u8 = b'\r';
pub const LF: u8 = b'\n';
