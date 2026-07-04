#![allow(dead_code)]
use std::vec::Vec;
mod access_control;
mod networking;
mod package;
use crate::access_control::application::{apply_groups, apply_users};
use crate::package::{Package, fetch_build_file, parse_pkgbuild};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct System {
    pub packages: Vec<Package>,
    pub hostname: String,
    pub users: Vec<access_control::user::User>,
    pub groups: Vec<access_control::group::Group>,
}

impl System {
    pub fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        apply_groups(&self.groups)?;
        apply_users(&self.users)?;
        for package in &self.packages {
            let raw = fetch_build_file(&package.name)?;
            let pkgbuild = parse_pkgbuild(&raw)?;
            pkgbuild.process()?;
        }
        networking::hostname::set_hostname(&self.hostname)?;
        Ok(())
    }
}
