#![allow(dead_code)]
use std::vec::Vec;
mod access_control;
mod networking;
mod package;
use crate::package::*;

pub struct System {
    packages: Vec<Package>,
    hostname: String,
    users: Vec<access_control::user::User>,
    groups: Vec<access_control::group::Group>,
}

impl System {
    pub fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        for package in &self.packages {
            package.install()?;
        }

        networking::hostname::set_hostname(&self.hostname)?;

        Ok(())
    }
}
