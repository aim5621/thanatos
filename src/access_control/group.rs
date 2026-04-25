use crate::access_control::user::User;

pub struct Group {
    pub name: String,
    pub gid: u32,
    members: Vec<User>,
}

impl Group {
    pub fn new(name: &str, gid: u32) -> Self {
        Group {
            name: name.to_string(),
            gid,
            members: Vec::new(),
        }
    }

    pub fn add_member(&mut self, user: User) {
        if !self.members.iter().any(|u| u.name == user.name) {
            self.members.push(user);
        }
    }

    pub fn remove_member(&mut self, username: &str) {
        self.members.retain(|u| u.name != username);
    }

    pub fn has_member(&self, username: &str) -> bool {
        self.members.iter().any(|u| u.name == username)
    }

    pub fn members(&self) -> &[User] {
        &self.members
    }
}
