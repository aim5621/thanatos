use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Group {
    pub name: String,
    pub gid: Option<u32>,
    members: Vec<String>,
}

impl Group {
    pub fn new(name: &str) -> Self {
        Group {
            name: name.to_string(),
            gid: None,
            members: Vec::new(),
        }
    }

    pub fn gid(mut self, gid: u32) -> Self {
        self.gid = Some(gid);
        self
    }

    pub fn add_member(&mut self, username: &str) {
        if !self.members.contains(&username.to_string()) {
            self.members.push(username.to_string());
        }
    }

    pub fn remove_member(&mut self, username: &str) {
        self.members.retain(|u| u != username);
    }

    pub fn has_member(&self, username: &str) -> bool {
        self.members.iter().any(|u| u == username)
    }

    pub fn members(&self) -> &[String] {
        &self.members
    }
}
