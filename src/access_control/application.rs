use crate::access_control::group::Group;
use crate::access_control::id_alloc::{IdAllocator, IdKind};
use crate::access_control::user::{Password, User};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const PASSWD_PATH: &str = "/etc/passwd";
const SHADOW_PATH: &str = "/etc/shadow";
const GROUP_PATH: &str = "/etc/group";

pub struct ResolvedUser {
    pub user: User,
    pub uid: u32,
    pub gid: u32,
}

pub struct ResolvedGroup {
    pub group: Group,
    pub gid: u32,
}

pub fn resolve_ids(
    users: &[User],
    groups: &[Group],
) -> Result<(Vec<ResolvedUser>, Vec<ResolvedGroup>), Box<dyn std::error::Error>> {
    let existing_uids = read_existing_ids(PASSWD_PATH)?;
    let existing_gids = read_existing_ids(GROUP_PATH)?;

    let mut uid_alloc = IdAllocator::with_existing(existing_uids);
    let mut gid_alloc = IdAllocator::with_existing(existing_gids);

    let mut resolved_groups = vec![];
    let mut group_name_to_gid: HashMap<String, u32> = HashMap::new();

    for group in groups {
        let gid = match group.gid {
            Some(id) => {
                let _ = gid_alloc.reserve(id);
                id
            }
            None => gid_alloc.allocate(IdKind::System)?,
        };
        group_name_to_gid.insert(group.name.clone(), gid);
        resolved_groups.push(ResolvedGroup {
            group: group.clone(),
            gid,
        });
    }

    let mut resolved_users = vec![];
    for user in users {
        let uid = match user.uid {
            Some(id) => {
                let _ = uid_alloc.reserve(id);
                id
            }
            None => uid_alloc.allocate(IdKind::User)?,
        };
        let gid = match user.primary_gid {
            Some(id) => id,
            None => uid_alloc.allocate(IdKind::User)?,
        };
        resolved_users.push(ResolvedUser {
            user: user.clone(),
            uid,
            gid,
        });
    }

    Ok((resolved_users, resolved_groups))
}

fn read_existing_ids(path: &str) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    if !Path::new(path).exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(path)?;
    let mut ids = vec![];
    for line in BufReader::new(file).lines() {
        let line = line?;
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() > 2 {
            if let Ok(id) = fields[2].parse::<u32>() {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

pub fn apply_users(users: &[ResolvedUser]) -> Result<(), Box<dyn std::error::Error>> {
    for ru in users {
        if user_exists(&ru.user.name)? {
            update_passwd_entry(ru)?;
            update_shadow_entry(&ru.user)?;
        } else {
            append_passwd_entry(ru)?;
            append_shadow_entry(&ru.user)?;
            create_home_dir(ru)?;
        }
    }
    Ok(())
}

pub fn apply_groups(groups: &[ResolvedGroup]) -> Result<(), Box<dyn std::error::Error>> {
    for rg in groups {
        if group_exists(&rg.group.name)? {
            update_group_entry(rg)?;
        } else {
            append_group_entry(rg)?;
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

fn append_passwd_entry(ru: &ResolvedUser) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(PASSWD_PATH)?;
    writeln!(
        file,
        "{}:x:{}:{}:{}:{}:{}",
        ru.user.name,
        ru.uid,
        ru.gid,
        ru.user.name,
        ru.user.home_dir,
        ru.user.shell.as_path()
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
        days_since_epoch()
    )?;
    Ok(())
}

fn append_group_entry(rg: &ResolvedGroup) -> Result<(), Box<dyn std::error::Error>> {
    let members = rg.group.members().join(",");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(GROUP_PATH)?;
    writeln!(file, "{}:x:{}:{}", rg.group.name, rg.gid, members)?;
    Ok(())
}

fn update_passwd_entry(ru: &ResolvedUser) -> Result<(), Box<dyn std::error::Error>> {
    let new_line = format!(
        "{}:x:{}:{}:{}:{}:{}",
        ru.user.name,
        ru.uid,
        ru.gid,
        ru.user.name,
        ru.user.home_dir,
        ru.user.shell.as_path()
    );
    replace_line_in_file(
        PASSWD_PATH,
        |line| line.split(':').next() == Some(&ru.user.name),
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
        days_since_epoch()
    );
    replace_line_in_file(
        SHADOW_PATH,
        |line| line.split(':').next() == Some(&user.name),
        &new_line,
    )
}

fn update_group_entry(rg: &ResolvedGroup) -> Result<(), Box<dyn std::error::Error>> {
    let members = rg.group.members().join(",");
    let new_line = format!("{}:x:{}:{}", rg.group.name, rg.gid, members);
    replace_line_in_file(
        GROUP_PATH,
        |line| line.split(':').next() == Some(&rg.group.name),
        &new_line,
    )
}

fn days_since_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400
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

fn create_home_dir(ru: &ResolvedUser) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(&ru.user.home_dir);
    if !path.exists() {
        fs::create_dir_all(path)?;
        std::process::Command::new("chown")
            .args([&format!("{}:{}", ru.uid, ru.gid), &ru.user.home_dir])
            .status()?;
        std::process::Command::new("chmod")
            .args(["700", &ru.user.home_dir])
            .status()?;
    }
    Ok(())
}
