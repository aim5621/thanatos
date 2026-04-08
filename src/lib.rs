#![allow(dead_code)]
use std::vec::Vec;

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

enum PackageType {
    Lib,
    Bin,
}

pub struct Package {
    name: String,
    path: String,
    r#type: PackageType,
    dependencies: Vec<Package>,
    build_deps: Vec<Package>,
}

pub struct System {
    packages: Vec<String>,
    hostname: String,
    users: Vec<User>,
    groups: Vec<Group>,
}
