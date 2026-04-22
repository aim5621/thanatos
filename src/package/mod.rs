mod build_file;
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
    pub fn install(&self, package_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let raw = build_file::fetch_build_file(package_name)?;
        let _pkgbuild = build_file::parse_pkgbuild(&raw)?;

        Ok(())
    }
}
