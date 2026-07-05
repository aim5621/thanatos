use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub name: String,
    pub uid: Option<u32>,
    pub primary_gid: Option<u32>,
    pub secondary_gids: Vec<u32>,
    pub home_dir: String,
    pub shell: Shell,
    pub password: Password,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Sh,
    Custom(String),
}

impl Shell {
    pub fn as_path(&self) -> &str {
        match self {
            Shell::Bash => "/bin/bash",
            Shell::Zsh => "/bin/zsh",
            Shell::Fish => "/usr/bin/fish",
            Shell::Sh => "/bin/sh",
            Shell::Custom(path) => path,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Password {
    Locked,
    Hashed(String),
}

impl User {
    pub fn new(name: &str) -> Self {
        User {
            name: name.to_string(),
            uid: None,
            primary_gid: None,
            secondary_gids: Vec::new(),
            home_dir: format!("/home/{}", name),
            shell: Shell::Bash,
            password: Password::Locked,
        }
    }

    pub fn uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    pub fn primary_group(mut self, gid: u32) -> Self {
        self.primary_gid = Some(gid);
        self
    }

    pub fn shell(mut self, shell: Shell) -> Self {
        self.shell = shell;
        self
    }

    pub fn home_dir(mut self, path: &str) -> Self {
        self.home_dir = path.to_string();
        self
    }

    pub fn password(mut self, password: Password) -> Self {
        self.password = password;
        self
    }

    pub fn add_group(&mut self, gid: u32) {
        if !self.secondary_gids.contains(&gid) {
            self.secondary_gids.push(gid);
        }
    }

    pub fn remove_group(&mut self, gid: u32) {
        self.secondary_gids.retain(|&g| g != gid);
    }

    pub fn in_group(&self, gid: u32) -> bool {
        self.primary_gid == Some(gid) || self.secondary_gids.contains(&gid)
    }
}
