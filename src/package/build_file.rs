use reqwest::blocking::get;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn fetch_build_file(package_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        package_name
    );

    let response = reqwest::blocking::get(&url)?;

    if response.status() == 404 {
        return Err(format!("package '{}' not found on AUR", package_name).into());
    }

    Ok(response.text()?)
}

pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: u32,
    pub depends: Vec<String>,
    pub make_depends: Vec<String>,
    pub sources: Vec<String>,
    pub checksums: Vec<String>,
    pub build_fn: String,
    pub package_fn: String,
}

pub fn parse_pkgbuild(content: &str) -> Result<PkgBuild, Box<dyn std::error::Error>> {
    let mut name = String::new();
    let mut version = String::new();
    let mut release = 1u32;
    let mut depends = vec![];
    let mut make_depends = vec![];
    let mut sources = vec![];
    let mut checksums = vec![];
    let mut build_fn = String::new();
    let mut package_fn = String::new();

    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        if line.starts_with("pkgname=") {
            name = line
                .trim_start_matches("pkgname=")
                .trim_matches('\'')
                .to_string();
        } else if line.starts_with("pkgver=") {
            version = line.trim_start_matches("pkgver=").to_string();
        } else if line.starts_with("pkgrel=") {
            release = line.trim_start_matches("pkgrel=").parse()?;
        } else if line.starts_with("depends=") {
            depends = parse_array(line, &mut lines);
        } else if line.starts_with("makedepends=") {
            make_depends = parse_array(line, &mut lines);
        } else if line.starts_with("source=") {
            sources = parse_array(line, &mut lines);
        } else if line.starts_with("sha256sums=") {
            checksums = parse_array(line, &mut lines);
        } else if line.starts_with("build()") {
            build_fn = parse_function(&mut lines);
        } else if line.starts_with("package()") {
            package_fn = parse_function(&mut lines);
        }
    }

    Ok(PkgBuild {
        name,
        version,
        release,
        depends,
        make_depends,
        sources,
        checksums,
        build_fn,
        package_fn,
    })
}

fn parse_array(line: &str, lines: &mut std::iter::Peekable<std::str::Lines>) -> Vec<String> {
    let mut result = vec![];
    let mut buf = line.to_string();

    while !buf.contains(')') {
        if let Some(next) = lines.next() {
            buf.push_str(next);
        } else {
            break;
        }
    }

    let inner = buf
        .split('(')
        .nth(1)
        .unwrap_or("")
        .split(')')
        .next()
        .unwrap_or("");
    for item in inner.split_whitespace() {
        let clean = item.trim_matches('\'').trim_matches('"').to_string();
        if !clean.is_empty() {
            result.push(clean);
        }
    }
    result
}

fn parse_function(lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let mut body = String::new();
    let mut depth = 0;

    for line in lines.by_ref() {
        if line.contains('{') {
            depth += 1;
        }
        if line.contains('}') {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        body.push_str(line);
        body.push('\n');
    }
    body
}

impl PkgBuild {
    pub fn process(&self) {}
}

pub enum Source {
    Tarball(String),
    Git(String),
}

pub fn parse_source(source: &str) -> Source {
    if source.contains("git+") || source.ends_with(".git") {
        Source::Git(source.trim_start_matches("git+").to_string())
    } else {
        Source::Tarball(source.to_string())
    }
}

pub fn fetch_source(source: &str, dest: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match parse_source(source) {
        Source::Tarball(url) => fetch_tarball(&url, dest),
        Source::Git(url) => fetch_git(&url, dest),
    }
}

fn fetch_tarball(url: &str, dest: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let filename = url.split('/').last().unwrap_or("source.tar.gz");
    let out_path = dest.join(filename);

    let bytes = get(url)?.bytes()?;
    let mut file = std::fs::File::create(&out_path)?;
    file.write_all(&bytes)?;

    Command::new("tar")
        .args([
            "-xf",
            out_path.to_str().unwrap(),
            "-C",
            dest.to_str().unwrap(),
        ])
        .status()?;

    Ok(out_path)
}

fn fetch_git(url: &str, dest: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Command::new("git")
        .args(["clone", "--depth=1", url, dest.to_str().unwrap()])
        .status()?;

    Ok(dest.to_path_buf())
}

pub fn run_build_fn(build_fn: &str, build_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let script_path = build_dir.join("thanatos_build.sh");
    let mut script = std::fs::File::create(&script_path)?;

    writeln!(script, "#!/bin/bash")?;
    writeln!(script, "set -e")?;
    writeln!(script, "cd {}", build_dir.to_str().unwrap())?;
    writeln!(script, "{}", build_fn)?;

    Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap()])
        .status()?;

    let status = Command::new("bash")
        .arg(script_path.to_str().unwrap())
        .current_dir(build_dir)
        .status()?;

    if !status.success() {
        return Err(format!("build failed with status: {}", status).into());
    }

    Ok(())
}
