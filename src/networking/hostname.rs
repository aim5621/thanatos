use std::fs;
use std::process::Command;

pub fn set_hostname(hostname: &str) -> Result<(), Box<dyn std::error::Error>> {
    validate_hostname(hostname)?;

    fs::write("/etc/hostname", hostname)?;

    match Command::new("hostname").arg(hostname).status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("warning: 'hostname' command exited with status {}", status),
        Err(e) => eprintln!("warning: could not run 'hostname' command: {}", e),
    }

    Ok(())
}

fn validate_hostname(hostname: &str) -> Result<(), Box<dyn std::error::Error>> {
    if hostname.is_empty() || hostname.len() > 63 {
        return Err("hostname must be between 1 and 63 characters".into());
    }

    if hostname.starts_with('-') {
        return Err("hostname must not start with a hyphen".into());
    }

    if !hostname
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("hostname must only contain lowercase a-z, 0-9, and hyphens".into());
    }

    Ok(())
}
