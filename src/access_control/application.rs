use crate::access_control::group::Group;
use crate::access_control::user::{Password, User};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const PASSWD_PATH: &str = "/etc/passwd";
const SHADOW_PATH: &str = "/etc/shadow";
const GROUP_PATH: &str = "/etc/group";

pub fn apply_users(users: &[User]) -> Result<(), Box<dyn std::error::Error>> {
    for user in users {
        if user_exists(&user.name)? {
            update_passwd_entry(user)?;
            update_shadow_entry(user)?;
        } else {
            append_passwd_entry(user)?;
            append_shadow_entry(user)?;
            create_home_dir(user)?;
        }
    }
    Ok(())
}

pub fn apply_groups(groups: &[Group]) -> Result<(), Box<dyn std::error::Error>> {
    for group in groups {
        if group_exists(&group.name)? {
            update_group_entry(group)?;
        } else {
            append_group_entry(group)?;
        }
    }
    Ok(())
}

pub fn remove_user(username: &str) -> Result<(), Box<dyn std::error::Error>> {
    remove_line_from_file(PASSWD_PATH, |line| line.split(':').next() == Some(username))?;
    remove_line_from_file(SHADOW_PATH, |line| line.split(':').next() == Some(username))?;
    Ok(())
}

pub fn remove_group(groupname: &str) -> Result<(), Box<dyn std::error::Error>> {
    remove_line_from_file(GROUP_PATH, |line| line.split(':').next() == Some(groupname))?;
    Ok(())
}

fn user_exists(username: &str) -> Result<bool, Box<dyn std::error::Error>> {
    line_exists(PASSWD_PATH, |line| line.split(':').next() == Some(username))
}

fn group_exists(groupname: &str) -> Result<bool, Box<dyn std::error::Error>> {
    line_exists(GROUP_PATH, |line| line.split(':').next() == Some(groupname))
}

fn line_exists(
    path: &str,
    predicate: impl Fn(&str) -> bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !Path::new(path).exists() {
        return Ok(false);
    }
    let file = fs::File::open(path)?;
    for line in BufReader::new(file).lines() {
        if predicate(&line?) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn append_passwd_entry(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(PASSWD_PATH)?;
    writeln!(
        file,
        "{}:x:{}:{}:{}:{}:{}",
        user.name,
        user.uid,
        user.primary_gid,
        user.name,
        user.home_dir,
        user.shell.as_path()
    )?;
    Ok(())
}

fn append_shadow_entry(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let password_field = match &user.password {
        Password::Locked => "!".to_string(),
        Password::Hashed(h) => h.clone(),
    };
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(SHADOW_PATH)?;
    writeln!(
        file,
        "{}:{}:{}:0:99999:7:::",
        user.name,
        password_field,
        days_since_epoch(),
    )?;
    Ok(())
}

fn append_group_entry(group: &Group) -> Result<(), Box<dyn std::error::Error>> {
    let members = group
        .members()
        .iter()
        .map(|u| u.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(GROUP_PATH)?;
    writeln!(file, "{}:x:{}:{}", group.name, group.gid, members)?;
    Ok(())
}

fn update_passwd_entry(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let new_line = format!(
        "{}:x:{}:{}:{}:{}:{}",
        user.name,
        user.uid,
        user.primary_gid,
        user.name,
        user.home_dir,
        user.shell.as_path()
    );
    replace_line_in_file(
        PASSWD_PATH,
        |line| line.split(':').next() == Some(&user.name),
        &new_line,
    )
}

fn update_shadow_entry(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let password_field = match &user.password {
        Password::Locked => "!".to_string(),
        Password::Hashed(h) => h.clone(),
    };
    let new_line = format!(
        "{}:{}:{}:0:99999:7:::",
        user.name,
        password_field,
        days_since_epoch(),
    );
    replace_line_in_file(
        SHADOW_PATH,
        |line| line.split(':').next() == Some(&user.name),
        &new_line,
    )
}

fn update_group_entry(group: &Group) -> Result<(), Box<dyn std::error::Error>> {
    let members = group
        .members()
        .iter()
        .map(|u| u.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let new_line = format!("{}:x:{}:{}", group.name, group.gid, members);
    replace_line_in_file(
        GROUP_PATH,
        |line| line.split(':').next() == Some(&group.name),
        &new_line,
    )
}

fn replace_line_in_file(
    path: &str,
    predicate: impl Fn(&str) -> bool,
    new_line: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let updated = content
        .lines()
        .map(|line| {
            if predicate(line) {
                new_line.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, updated + "\n")?;
    Ok(())
}

fn remove_line_from_file(
    path: &str,
    predicate: impl Fn(&str) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(path).exists() {
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    let updated = content
        .lines()
        .filter(|line| !predicate(line))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, updated + "\n")?;
    Ok(())
}

fn create_home_dir(user: &User) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&user.home_dir);
    if !path.exists() {
        fs::create_dir_all(path)?;
        std::process::Command::new("chown")
            .args([
                &format!("{}:{}", user.uid, user.primary_gid),
                &user.home_dir,
            ])
            .status()?;
        std::process::Command::new("chmod")
            .args(["700", &user.home_dir])
            .status()?;
    }
    Ok(())
}

fn days_since_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400
}
