#![allow(dead_code)]
use std::vec::Vec;
mod access_control;
mod networking;
mod package;
use crate::access_control::application::{apply_groups, apply_users};
use crate::package::{Package, PackageFormat, fetch_build_file, parse_pkgbuild};

pub struct System {
    packages: Vec<Package>,
    hostname: String,
    users: Vec<access_control::user::User>,
    groups: Vec<access_control::group::Group>,
}

impl System {
    pub fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        apply_groups(&self.groups)?;
        apply_users(&self.users)?;
        for package in &self.packages {
            
            match package.format {
                PackageFormat::Deb => Ok(()),
                PackageFormat::Tar => Ok(()),
                PackageFormat::AUR => {
                    let raw = fetch_build_file(&package.name)?;
                    let pkgbuild = parse_pkgbuild(&raw)?;
                    pkgbuild.process()
                },
                PackageFormat::Rpm => Ok(()),
                PackageFormat::Appimage => Ok(()),
                PackageFormat::Pending => Err("Not set".into()),
            }?
        }
        networking::hostname::set_hostname(&self.hostname)?;
        Ok(())
    }
}
