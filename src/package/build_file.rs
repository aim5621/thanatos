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
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) Thanatos/0.1.0")
            .build()
            .expect("failed to build http client")
    })
}

pub fn fetch_build_file(package_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        package_name
    );

    let response = http_client().get(&url).send()?;

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
        } else if line.starts_with("md5sums=") {
            checksums = vec!["SKIP".to_string(); parse_array(line, &mut lines).len()];
        } else if line.starts_with("build()") {
            build_fn = parse_function(&mut lines);
        } else if line.starts_with("package()") {
            package_fn = parse_function(&mut lines);
        }
    }

    sources = sources
        .into_iter()
        .map(|s| {
            s.replace("${pkgname}", &name)
                .replace("$pkgname", &name)
                .replace("${pkgver}", &version)
                .replace("$pkgver", &version)
        })
        .collect();

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
    pub fn process(&self) -> Result<(), Box<dyn std::error::Error>> {
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
            run_build_fn(&self.build_fn, &build_dir)?;
        }

        if !self.package_fn.is_empty() {
            run_package_fn(&self.package_fn, &build_dir, &staging_dir)?;
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
            run_build_fn(&self.build_fn, &build_dir)?;
        }

        if !self.package_fn.is_empty() {
            run_package_fn(&self.package_fn, &build_dir, &staging_dir)?;
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
            match fetch_build_file(dep_name) {
                Ok(raw) => {
                    let dep_pkgbuild = parse_pkgbuild(&raw)?;
                    dep_pkgbuild.process_as_dependency()?;
                }
                Err(_) => {
                    println!("'{}' not found on AUR, trying pacman...", dep_name);
                    install_via_pacman(dep_name)?;
                }
            }
        }

        Ok(())
    }
}

fn install_via_pacman(package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("pacman")
        .args(["-S", "--noconfirm", "--needed", package_name])
        .status()?;

    if !status.success() {
        return Err(format!(
            "failed to install '{}' via pacman (not found on AUR or in official repos)",
            package_name
        )
        .into());
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

    let response = http_client()
        .get(url)
        .send()
        .map_err(|e| format!("failed to fetch tarball from '{}': {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!("fetching '{}' returned HTTP {}", url, response.status()).into());
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("failed to read response body from '{}': {}", url, e))?;

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

fn find_source_dir(build_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(build_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            return Ok(entry.path());
        }
    }
    Ok(build_dir.to_path_buf())
}

pub fn run_build_fn(build_fn: &str, build_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let script_path = build_dir.join("thanatos_build.sh");
    let mut script = std::fs::File::create(&script_path)?;

    let src_dir = find_source_dir(build_dir)?;

    writeln!(script, "#!/bin/bash")?;
    writeln!(script, "set -e")?;
    writeln!(script, "cd {}", src_dir.to_str().unwrap())?;
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
) -> Result<(), Box<dyn std::error::Error>> {
    let script_path = build_dir.join("thanatos_package.sh");
    let mut script = std::fs::File::create(&script_path)?;

    let src_dir = find_source_dir(build_dir)?;

    writeln!(script, "#!/bin/bash")?;
    writeln!(script, "set -e")?;
    writeln!(script, "pkgdir={}", staging_dir.to_str().unwrap())?;
    writeln!(script, "cd {}", src_dir.to_str().unwrap())?;
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
