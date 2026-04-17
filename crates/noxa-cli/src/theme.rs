//! Terminal color and formatting theme — single source of truth for all CLI output.
//!
//! Color roles:
//!   green  — success, ✓
//!   yellow — warnings, errors, ✗
//!   cyan   — URLs, commands, primary labels
//!   pink   — file paths
//!   blue   — secondary URLs (list views)
//!   dim    — secondary text, hints, metadata
//!   bold   — emphasis, counts, values
#![allow(non_upper_case_globals)]

// ── Core ANSI codes ──────────────────────────────────────────────────────────
pub const reset: &str = "\x1b[0m";
pub const bold: &str = "\x1b[1m";
pub const dim: &str = "\x1b[2m";
pub const green: &str = "\x1b[92m";
pub const yellow: &str = "\x1b[93m";
pub const cyan: &str = "\x1b[96m";
pub const pink: &str = "\x1b[95m";
pub const blue: &str = "\x1b[94m";
// ── Prefix helpers ───────────────────────────────────────────────────────────
pub fn warning(msg: &str) -> String {
    format!("{yellow}warning:{reset} {msg}")
}
pub fn info(msg: &str) -> String {
    format!("{cyan}info:{reset} {msg}")
}
