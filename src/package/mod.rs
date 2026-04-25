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
