use crate::package::db::{Package, PackageDb};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static CLIENT: OnceLock<Client> = OnceLock::new();

fn http_client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("failed to build http client")
    })
}

pub fn fetch_build_file(package_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        package_name
    );

    let output = std::process::Command::new("curl")
        .args(["-s", "-f", &url])
        .output()?;

    if !output.status.success() {
        return Err(format!("curl failed to fetch PKGBUILD for '{}'", package_name).into());
    }

    let body = String::from_utf8(output.stdout)?;

    if body.contains("<!doctype html") || body.contains("<html") {
        return Err(format!("AUR returned HTML for '{}' (bot protection)", package_name).into());
    }

    Ok(body)
}

//    pub fn fetch_build_file(package_name: &str) -> Result<String, Box<dyn std::error::Error>> {
//        let url = format!(
//            "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
//            package_name
//        );
//
//        let response = http_client().get(&url).send()?;
//
//        if response.status() == 404 {
//            return Err(format!("package '{}' not found on AUR", package_name).into());
//        }
//
//        let body = response.text()?;
//
//        if body.contains("<!doctype html") || body.contains("<html") {
//            return Err(format!(
//                "AUR returned an HTML page instead of PKGBUILD for '{}' (likely bot protection)",
//                package_name
//            )
//            .into());
//        }
//
//        Ok(body)
//    }

pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: u32,
    pub url: String,
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
    let mut url = String::new();

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
        } else if line.starts_with("md5sums=") || line.starts_with("sha512sums=") {
            checksums = vec!["SKIP".to_string(); parse_array(line, &mut lines).len()];
        } else if line.starts_with("url=") {
            url = line
                .trim_start_matches("url=")
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
        } else if line.starts_with("build()") {
            build_fn = parse_function(&mut lines);
        } else if line.starts_with("package()") {
            package_fn = parse_function(&mut lines);
        }
    }

    sources = sources
        .into_iter()
        .map(|s| {
            let expanded = s
                .replace("${pkgname}", &name)
                .replace("$pkgname", &name)
                .replace("${pkgver}", &version)
                .replace("$pkgver", &version)
                .replace("${url}", &url)
                .replace("$url", &url);

            if let Some(idx) = expanded.find("::") {
                expanded[idx + 2..].to_string()
            } else {
                expanded
            }
        })
        .collect();

    Ok(PkgBuild {
        name,
        version,
        release,
        url,
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
    pub fn process(&self) -> Result<(), Box<dyn std::error::Error>> {
        let build_dir = PathBuf::from(format!("/tmp/thanatos/{}-{}", self.name, self.version));

        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir)?;
        }
        let staging_dir = build_dir.join("pkg");

        std::fs::create_dir_all(&build_dir)?;
        std::fs::create_dir_all(&staging_dir)?;

        let mut db = PackageDb::load()?;

        if db.is_installed(&self.name) {
            println!("package '{}' is already installed", self.name);
            return Ok(());
        }

        eprintln!("sources: {:?}", self.sources);
        eprintln!("checksums: {:?}", self.checksums);

        self.resolve_dependencies()?;

        let build_dir = PathBuf::from(format!("/tmp/thanatos/{}-{}", self.name, self.version));
        let staging_dir = build_dir.join("pkg");

        std::fs::create_dir_all(&build_dir)?;
        std::fs::create_dir_all(&staging_dir)?;

        for (source, checksum) in self.sources.iter().zip(self.checksums.iter()) {
            if !source.starts_with("http") && !source.starts_with("git+") {
                eprintln!("skipping unsupported source syntax: {}", source);
                continue;
            }
            let fetched =
                fetch_source(source, &build_dir).map_err(|e| -> Box<dyn std::error::Error> {
                    format!("package '{}': {}", self.name, e).into()
                })?;
            verify_checksum(&fetched, checksum)?;
        }

        if !self.build_fn.is_empty() {
            run_build_fn(&self.build_fn, &build_dir, &self.name, &self.version)?;
        }

        if !self.package_fn.is_empty() {
            run_package_fn(
                &self.package_fn,
                &build_dir,
                &staging_dir,
                &self.name,
                &self.version,
            )?;
        }

        let installed_files = collect_files(&staging_dir)?;

        install_from_staging(&staging_dir)?;

        let mut pkg = Package::new(&self.name);
        pkg.depends = self.depends.clone();
        pkg.mark_installed(
            self.version.clone(),
            self.release,
            installed_files,
            crate::package::db::InstallReason::Explicit,
        );
        db.insert(pkg);

        db.save()?;

        std::fs::remove_dir_all(&build_dir)?;

        println!("installed {}-{}", self.name, self.version);

        Ok(())
    }

    fn process_as_dependency(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut db = PackageDb::load()?;

        if db.is_installed(&self.name) {
            return Ok(());
        }

        self.resolve_dependencies()?;

        let build_dir = PathBuf::from(format!("/tmp/thanatos/{}-{}", self.name, self.version));
        let staging_dir = build_dir.join("pkg");

        std::fs::create_dir_all(&build_dir)?;
        std::fs::create_dir_all(&staging_dir)?;

        for (source, checksum) in self.sources.iter().zip(self.checksums.iter()) {
            if !source.starts_with("http") && !source.starts_with("git+") {
                continue;
            }
            let fetched = fetch_source(source, &build_dir)?;
            verify_checksum(&fetched, checksum)?;
        }

        if !self.build_fn.is_empty() {
            run_build_fn(&self.build_fn, &build_dir, &self.name, &self.version)?;
        }

        if !self.package_fn.is_empty() {
            run_package_fn(
                &self.package_fn,
                &build_dir,
                &staging_dir,
                &self.name,
                &self.version,
            )?;
        }

        let installed_files = collect_files(&staging_dir)?;
        install_from_staging(&staging_dir)?;

        let mut pkg = Package::new(&self.name);
        pkg.depends = self.depends.clone();
        pkg.mark_installed(
            self.version.clone(),
            self.release,
            installed_files,
            crate::package::db::InstallReason::Dependency,
        );
        db.insert(pkg);
        db.save()?;

        std::fs::remove_dir_all(&build_dir)?;
        println!("installed dependency {}-{}", self.name, self.version);

        Ok(())
    }

    fn resolve_dependencies(&self) -> Result<(), Box<dyn std::error::Error>> {
        let db = PackageDb::load()?;

        for dep_name in &self.depends {
            if db.is_installed(dep_name) {
                continue;
            }

            println!("resolving dependency '{}' for '{}'", dep_name, self.name);

            if is_available_in_pacman(dep_name) {
                install_via_pacman(dep_name)?;
                continue;
            }

            println!("'{}' not found in official repos, trying AUR...", dep_name);
            match fetch_build_file(dep_name) {
                Ok(raw) => {
                    let dep_pkgbuild = parse_pkgbuild(&raw)?;
                    dep_pkgbuild.process_as_dependency()?;
                }
                Err(e) => {
                    return Err(format!(
                        "dependency '{}' not found in pacman or AUR: {}",
                        dep_name, e
                    )
                    .into());
                }
            }
        }

        Ok(())
    }
}

fn is_available_in_pacman(package_name: &str) -> bool {
    Command::new("pacman")
        .args(["-Si", package_name])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn install_via_pacman(package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("pacman")
        .args(["-S", "--noconfirm", "--needed", package_name])
        .status()?;

    if !status.success() {
        return Err(format!("failed to install '{}' via pacman", package_name).into());
    }

    println!("installed '{}' via pacman", package_name);
    Ok(())
}

pub(crate) fn collect_files(staging_dir: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut files = vec![];

    for entry in walkdir::WalkDir::new(staging_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let relative = entry.path().strip_prefix(staging_dir)?;
            files.push(format!("/{}", relative.to_str().unwrap()));
        }
    }

    Ok(files)
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
        Source::Tarball(url) => fetch_tarball(&url, dest)
            .map_err(|e| format!("tarball source '{}' failed: {}", source, e).into()),
        Source::Git(url) => fetch_git(&url, dest)
            .map_err(|e| format!("git source '{}' failed: {}", source, e).into()),
    }
}

fn fetch_tarball(url: &str, dest: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let filename = url.split('/').last().unwrap_or("source.tar.gz");
    let out_path = dest.join(filename);

    let status = Command::new("curl")
        .args(["-f", "-L", "-o", out_path.to_str().unwrap(), url])
        .status()?;

    if !status.success() {
        return Err(format!("curl failed to fetch tarball from '{}'", url).into());
    }

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

fn find_source_dir(build_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(build_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        eprintln!(
            "DEBUG: found entry '{}' in build_dir",
            name.to_string_lossy()
        );
        if entry.file_type()?.is_dir() && name.to_string_lossy() != "pkg" {
            eprintln!("DEBUG: selected source dir: {}", entry.path().display());
            return Ok(entry.path());
        }
    }
    eprintln!("DEBUG: no suitable source dir found, falling back to build_dir itself");
    Ok(build_dir.to_path_buf())
}

pub fn run_build_fn(
    build_fn: &str,
    build_dir: &Path,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let script_path = build_dir.join("thanatos_build.sh");
    let mut script = std::fs::File::create(&script_path)?;

    writeln!(script, "#!/bin/bash")?;
    writeln!(script, "set -e")?;
    writeln!(script, "pkgname={}", name)?;
    writeln!(script, "pkgver={}", version)?;
    writeln!(script, "srcdir={}", build_dir.to_str().unwrap())?;
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

fn sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub(crate) fn verify_checksum(
    path: &Path,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if expected == "SKIP" {
        return Ok(());
    }

    let bytes = std::fs::read(path)?;
    let hash = sha256(&bytes);

    if hash != expected {
        return Err(format!(
            "checksum mismatch for {}: expected {}, got {}",
            path.display(),
            expected,
            hash
        )
        .into());
    }

    Ok(())
}

pub fn run_package_fn(
    package_fn: &str,
    build_dir: &Path,
    staging_dir: &Path,
    name: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let script_path = build_dir.join("thanatos_package.sh");
    let mut script = std::fs::File::create(&script_path)?;

    writeln!(script, "#!/bin/bash")?;
    writeln!(script, "set -e")?;
    writeln!(script, "pkgname={}", name)?;
    writeln!(script, "pkgver={}", version)?;
    writeln!(script, "srcdir={}", build_dir.to_str().unwrap())?;
    writeln!(script, "pkgdir={}", staging_dir.to_str().unwrap())?;
    writeln!(script, "cd {}", build_dir.to_str().unwrap())?;
    writeln!(script, "{}", package_fn)?;

    Command::new("chmod")
        .args(["+x", script_path.to_str().unwrap()])
        .status()?;

    let status = Command::new("bash")
        .arg(script_path.to_str().unwrap())
        .current_dir(build_dir)
        .status()?;

    if !status.success() {
        return Err(format!("package() failed with status: {}", status).into());
    }

    Ok(())
}

fn install_from_staging(staging_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("cp")
        .args(["-r", staging_dir.to_str().unwrap(), "/"])
        .status()?;

    if !status.success() {
        return Err("failed to install files from staging to root".into());
    }

    Ok(())
}
