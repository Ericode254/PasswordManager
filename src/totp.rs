//! pass-otp compatible URIs; oathtool performs the cryptography.
use crate::pass::commands;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

pub struct Token {
    secret: Zeroizing<String>,
    period: u64,
    digits: usize,
    algorithm: String,
}

pub struct Code {
    pub value: Zeroizing<String>,
    pub valid_from: u64,
    pub expires_at: u64,
    deadline: Instant,
}

impl Code {
    pub fn remaining(&self) -> Duration {
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return Duration::ZERO;
        };
        if now.as_secs() < self.valid_from {
            return Duration::ZERO;
        }
        Duration::from_secs(self.expires_at)
            .saturating_sub(now)
            .min(self.deadline.saturating_duration_since(Instant::now()))
    }
}

pub fn parse(content: &str) -> Result<Token, String> {
    let mut uris = content
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("otpauth://"));
    let uri = uris.next().ok_or("No OTP URI found. Add an otpauth://totp/... URI on its own line in Notes, or use pass otp append.")?;
    if uris.next().is_some() {
        return Err("Multiple OTP URIs found. Keep one token per entry.".into());
    }
    let body = uri
        .strip_prefix("otpauth://totp/")
        .ok_or("Only TOTP is supported; HOTP counters are never advanced.")?;
    let (label, query) = body.split_once('?').ok_or("Invalid TOTP URI")?;
    if label.is_empty() || uri.contains('#') || uri.chars().any(char::is_control) {
        return Err("Invalid TOTP URI".into());
    }
    let mut token = Token {
        secret: Zeroizing::new(String::new()),
        period: 30,
        digits: 6,
        algorithm: "SHA1".into(),
    };
    let mut seen = std::collections::BTreeSet::new();
    for param in query.split('&') {
        let (key, value) = param.split_once('=').ok_or("Invalid TOTP parameter")?;
        if !seen.insert(key) {
            return Err("Duplicate TOTP parameter".into());
        }
        match key {
            "secret" => token.secret = decode_secret(value)?,
            "period" => token.period = value.parse().map_err(|_| "Invalid TOTP period")?,
            "digits" => token.digits = value.parse().map_err(|_| "Invalid TOTP digits")?,
            "algorithm" => token.algorithm = value.to_ascii_uppercase(),
            "counter" => return Err("HOTP counters are not supported".into()),
            "issuer" => (),
            _ => return Err("Unsupported TOTP parameter".into()),
        }
    }
    if token.secret.is_empty()
        || !token
            .secret
            .chars()
            .all(|c| c.is_ascii_uppercase() || matches!(c, '2'..='7' | '='))
    {
        return Err("TOTP secret must use Base32 encoding".into());
    }
    if !(1..=86400).contains(&token.period)
        || !matches!(token.digits, 6 | 8)
        || !matches!(token.algorithm.as_str(), "SHA1" | "SHA256" | "SHA512")
    {
        return Err(
            "Use a 1–86400 second period, 6 or 8 digits, and SHA1, SHA256 or SHA512.".into(),
        );
    }
    Ok(token)
}

fn decode_secret(value: &str) -> Result<Zeroizing<String>, String> {
    let mut output = Zeroizing::new(String::new());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        let byte = if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or("Invalid encoded TOTP secret")?;
            let low = bytes
                .next()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or("Invalid encoded TOTP secret")?;
            (high * 16 + low) as u8
        } else {
            byte
        };
        output.push(char::from(byte).to_ascii_uppercase());
    }
    Ok(output)
}

pub fn generate(path: &str) -> Result<Code, String> {
    let entry = commands::pass_show(path)?;
    let token = parse(&entry.content)?;
    drop(entry);
    for _ in 0..2 {
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock predates Unix epoch")?;
        let mut child = Command::new("oathtool")
            .args([
                format!("--totp={}", token.algorithm),
                "--base32".into(),
                format!("--digits={}", token.digits),
                format!("--time-step-size={}s", token.period),
                "-".into(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(
                |_| "Cannot run oathtool. Install OATH Toolkit (also required by pass-otp).",
            )?;
        let mut stdin = child.stdin.take().ok_or("TOTP input unavailable")?;
        let written = stdin
            .write_all(token.secret.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"));
        drop(stdin);
        let output = child
            .wait_with_output()
            .map_err(|_| "TOTP generator stopped")?;
        let bytes = Zeroizing::new(output.stdout);
        if !output.status.success() || written.is_err() {
            return Err("TOTP generation failed. Check the URI and oathtool installation.".into());
        }
        let value = Zeroizing::new(
            std::str::from_utf8(&bytes)
                .map_err(|_| "Invalid TOTP output")?
                .trim()
                .to_string(),
        );
        if value.len() != token.digits || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Invalid TOTP output".into());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Invalid system clock")?;
        if started.as_secs() / token.period != now.as_secs() / token.period {
            continue;
        }
        let valid_from = now.as_secs() / token.period * token.period;
        let expires_at = valid_from + token.period;
        let remaining = Duration::from_secs(expires_at).saturating_sub(now);
        return Ok(Code {
            value,
            valid_from,
            expires_at,
            deadline: Instant::now() + remaining,
        });
    }
    Err("Code expired during generation. Press r to retry.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_pass_otp_notes_and_custom_parameters_without_leaking_errors() {
        let token = parse("password\nusername: alice\notpauth://totp/Test:alice?secret=gezdgnbv%3D%3D&digits=8&period=60&algorithm=SHA256").unwrap();
        assert_eq!(token.secret.as_str(), "GEZDGNBV==");
        assert_eq!(
            (token.period, token.digits, token.algorithm.as_str()),
            (60, 8, "SHA256")
        );
        for uri in [
            "otpauth://hotp/Test?secret=PRIVATE&counter=0",
            "otpauth://totp/Test?secret=PRIVATE&period=0",
            "otpauth://totp/Test?secret=PRIVATE&digits=99",
            "otpauth://totp/Test?secret=PRIVATE&secret=OTHER",
            "otpauth://totp/Test?secret=PRIVATE&algorithm=MD5",
        ] {
            let error = parse(uri).err().unwrap();
            assert!(!error.contains("PRIVATE"));
        }
        assert!(parse("otpauth://totp/A?secret=AA\notpauth://totp/B?secret=BB").is_err());
    }
    #[test]
    fn expired_and_future_codes_cannot_be_copied() {
        let mut code = Code {
            value: Zeroizing::new("123456".into()),
            valid_from: 0,
            expires_at: 1,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        assert!(code.remaining().is_zero());
        code.valid_from = u64::MAX;
        code.expires_at = u64::MAX;
        assert!(code.remaining().is_zero());
    }
}
