use std::{fs::OpenOptions, io::Write};

pub fn set_hostname(hostname: String) -> Result<(), Box<dyn std::error::Error>> {
    validate_hostname(&hostname)?;

    let mut hosts_file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open("/etc/hostname")?;

    hosts_file.write_all(hostname.as_bytes())?;

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
