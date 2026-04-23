mod build_file;
use crate::package::build_file::*;
use std::path::PathBuf;
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

        let build_dir = PathBuf::from(format!("/tmp/thanatos/{}", self.name));
        std::fs::create_dir_all(&build_dir)?;

        for source in &pkgbuild.sources {
            fetch_source(source, &build_dir)?;
        }

        run_build_fn(&pkgbuild.build_fn, &build_dir)?;

        Ok(())
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
        let build_dir = PathBuf::from("/tmp/thanatos/test_build");
        fs::create_dir_all(&build_dir).unwrap();
        let result = run_build_fn("echo 'build ok'", &build_dir);
        assert!(result.is_ok());
        fs::remove_dir_all(&build_dir).unwrap();
    }

    #[test]
    fn test_run_build_fn_failure() {
        let build_dir = PathBuf::from("/tmp/thanatos/test_build_fail");
        fs::create_dir_all(&build_dir).unwrap();
        let result = run_build_fn("exit 1", &build_dir);
        assert!(result.is_err());
        fs::remove_dir_all(&build_dir).unwrap();
    }
}
