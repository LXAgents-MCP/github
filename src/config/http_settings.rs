//! Where the HTTP transport listens: `PORT` and `HOST`, read when called.
use super::config::{AUTH_ENV, DEFAULT_HOST, DEFAULT_PORT, LOCAL_HOST};
use crate::services::context::Env;

/// Where the HTTP transport listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSettings {
    pub port: u16,
    pub host: String,
}

/// Read `PORT` and `HOST`: 3000 when `PORT` is unset or blank, as in `shared-instruction`. When `HOST` is
/// unset or blank it follows the access token: `0.0.0.0` when `MCP_AUTH_TOKEN` is set, as in
/// `shared-instruction`, and `127.0.0.1` when it is not, so a local install needs neither. Unlike its
/// `Number(PORT) || 3000`, a `PORT` that is set and is not a port is an error and not a silent 3000, and
/// `0` is accepted and asks the system for a free port.
pub fn http_settings(env: &dyn Env) -> Result<HttpSettings, String> {
    let raw = env.var("PORT").unwrap_or_default().trim().to_string();
    let mut port = DEFAULT_PORT;
    if !raw.is_empty() {
        port = parse_port(&raw)
            .ok_or_else(|| format!("PORT must be a whole number from 0 to 65535, got '{raw}'."))?;
    }
    let token = env.var(AUTH_ENV).unwrap_or_default().trim().to_string();
    let host = env.var("HOST").unwrap_or_default().trim().to_string();
    let host = if !host.is_empty() {
        host
    } else if token.is_empty() {
        LOCAL_HOST.to_string()
    } else {
        DEFAULT_HOST.to_string()
    };
    Ok(HttpSettings { port, host })
}

/// A port as JavaScript's `Number()` would read it and `Number.isInteger` accept it: decimal digits, an
/// optional fraction of zeros, or a hexadecimal, octal or binary literal.
fn parse_port(raw: &str) -> Option<u16> {
    let lower = raw.to_ascii_lowercase();
    let value: f64 = if let Some(hex) = lower.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()? as f64
    } else if let Some(oct) = lower.strip_prefix("0o") {
        i64::from_str_radix(oct, 8).ok()? as f64
    } else if let Some(bin) = lower.strip_prefix("0b") {
        i64::from_str_radix(bin, 2).ok()? as f64
    } else {
        if !raw
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
        {
            return None;
        }
        raw.parse::<f64>().ok()?
    };
    if value.fract() != 0.0 || !(0.0..=65535.0).contains(&value) {
        return None;
    }
    Some(value as u16)
}
