pub struct User {
    pub name: String,
    pub uid: u32,
    pub primary_gid: u32,
    pub secondary_gids: Vec<u32>,
    pub home_dir: String,
    pub shell: Shell,
    pub password: Password,
}

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

pub enum Password {
    Locked,
    Hashed(String),
}

impl User {
    pub fn new(name: &str, uid: u32, primary_gid: u32) -> Self {
        User {
            name: name.to_string(),
            uid,
            primary_gid,
            secondary_gids: Vec::new(),
            home_dir: format!("/home/{}", name),
            shell: Shell::Bash,
            password: Password::Locked,
        }
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
        self.primary_gid == gid || self.secondary_gids.contains(&gid)
    }
}
