mod build_file;
mod db;
use crate::package::build_file::*;

enum PackageType {
    Lib,
    Bin,
}

pub struct Package {
    pub name: String,
    path: String,
    r#type: PackageType,
    dependencies: Vec<Package>,
    build_deps: Vec<Package>,
}

impl Package {
    pub fn install(&self) -> Result<(), Box<dyn std::error::Error>> {
        let raw = fetch_build_file(&self.name)?;
        let pkgbuild = parse_pkgbuild(&raw)?;

        let build_dir = std::env::temp_dir()
            .join("thanatos")
            .join("build")
            .join(&self.name);

        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir)?;
        }
        std::fs::create_dir_all(&build_dir)?;

        let result = (|| {
            for source in &pkgbuild.sources {
                fetch_source(source, &build_dir)?;
            }
            run_build_fn(&pkgbuild.build_fn, &build_dir)?;
            Ok(())
        })();

        let _ = std::fs::remove_dir_all(&build_dir);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_package(name: &str) -> Package {
        Package {
            name: name.to_string(),
            path: String::new(),
            r#type: PackageType::Bin,
            dependencies: vec![],
            build_deps: vec![],
        }
    }

    #[test]
    fn test_fetch_build_file_valid_package() {
        let result = fetch_build_file("hello");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("pkgname"));
        assert!(content.contains("pkgver"));
    }

    #[test]
    fn test_fetch_build_file_invalid_package() {
        let result = fetch_build_file("this-package-does-not-exist-xyz-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pkgbuild_fields() {
        let fake_pkgbuild = r#"
pkgname=hello
pkgver=2.12
pkgrel=1
depends=('glibc')
makedepends=('gcc')
source=('https://ftp.gnu.org/gnu/hello/hello-2.12.tar.gz')
sha256sums=('abc123')

build() {
    ./configure
    make
}

package() {
    make install
}
"#;
        let result = parse_pkgbuild(fake_pkgbuild);
        assert!(result.is_ok());
        let pkgbuild = result.unwrap();
        assert_eq!(pkgbuild.name, "hello");
        assert_eq!(pkgbuild.version, "2.12");
        assert_eq!(pkgbuild.release, 1);
        assert!(pkgbuild.depends.contains(&"glibc".to_string()));
        assert!(pkgbuild.make_depends.contains(&"gcc".to_string()));
        assert!(pkgbuild.build_fn.contains("make"));
        assert!(pkgbuild.package_fn.contains("make install"));
    }

    #[test]
    fn test_parse_source_tarball() {
        match parse_source("https://example.com/pkg.tar.gz") {
            Source::Tarball(url) => assert_eq!(url, "https://example.com/pkg.tar.gz"),
            Source::Git(_) => panic!("expected tarball"),
        }
    }

    #[test]
    fn test_parse_source_git() {
        match parse_source("git+https://github.com/example/repo.git") {
            Source::Git(url) => assert!(url.contains("github.com")),
            Source::Tarball(_) => panic!("expected git"),
        }
    }

    #[test]
    fn test_run_build_fn_success() {
        let build_dir = std::env::temp_dir().join("thanatos_test_build");
        fs::create_dir_all(&build_dir).unwrap();
        let result = run_build_fn("echo 'build ok'", &build_dir);
        assert!(result.is_ok());
        fs::remove_dir_all(&build_dir).unwrap();
    }

    #[test]
    fn test_run_build_fn_failure() {
        let build_dir = std::env::temp_dir().join("thanatos_test_build_fail");
        fs::create_dir_all(&build_dir).unwrap();
        let result = run_build_fn("exit 1", &build_dir);
        assert!(result.is_err());
        fs::remove_dir_all(&build_dir).unwrap();
    }
}

#[test]
fn test_verify_checksum_skip() {
    let tmp = std::env::temp_dir().join("thanatos_test_checksum");
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("dummy.txt");
    std::fs::write(&file, b"hello").unwrap();
    let result = verify_checksum(&file, "SKIP");
    assert!(result.is_ok());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn test_verify_checksum_valid() {
    let tmp = std::env::temp_dir().join("thanatos_test_checksum_valid");
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("dummy.txt");
    std::fs::write(&file, b"hello").unwrap();

    let hash = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::new().chain_update(b"hello").finalize())
    };

    let result = verify_checksum(&file, &hash);
    assert!(result.is_ok());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn test_verify_checksum_invalid() {
    let tmp = std::env::temp_dir().join("thanatos_test_checksum_invalid");
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("dummy.txt");
    std::fs::write(&file, b"hello").unwrap();
    let result = verify_checksum(&file, "notarealhash");
    assert!(result.is_err());
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn test_run_package_fn_success() {
    let build_dir = std::env::temp_dir().join("thanatos_test_package_fn");
    let staging_dir = build_dir.join("pkg");
    std::fs::create_dir_all(&staging_dir).unwrap();
    let result = run_package_fn("touch $pkgdir/installed.txt", &build_dir, &staging_dir);
    assert!(result.is_ok());
    assert!(staging_dir.join("installed.txt").exists());
    std::fs::remove_dir_all(&build_dir).unwrap();
}

#[test]
fn test_run_package_fn_failure() {
    let build_dir = std::env::temp_dir().join("thanatos_test_package_fn_fail");
    let staging_dir = build_dir.join("pkg");
    std::fs::create_dir_all(&staging_dir).unwrap();
    let result = run_package_fn("exit 1", &build_dir, &staging_dir);
    assert!(result.is_err());
    std::fs::remove_dir_all(&build_dir).unwrap();
}

#[test]
fn test_collect_files() {
    let tmp = std::env::temp_dir().join("thanatos_test_collect");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("a.txt"), b"a").unwrap();
    std::fs::write(tmp.join("b.txt"), b"b").unwrap();

    let files = collect_files(&tmp).unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"/a.txt".to_string()));
    assert!(files.contains(&"/b.txt".to_string()));
    std::fs::remove_dir_all(&tmp).unwrap();
}
