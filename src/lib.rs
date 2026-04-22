#![allow(dead_code)]
use std::vec::Vec;
mod package;
use crate::package::*;

pub struct User {
    name: String,
    primary_group: Group,
    secondary_groups: Vec<Group>,
    uid: u32,
    home_dir: String,
    shell: String,
}

pub struct Group {
    name: String,
    gid: u32,
    group_list: Vec<User>,
}

pub struct System {
    packages: Vec<Package>,
    hostname: String,
    users: Vec<User>,
    groups: Vec<Group>,
}

impl System {
    pub fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        for package in &self.packages {
            package.install(&package.name)?;
        }
        //TODO: STUB
        Ok(())
    }
}
