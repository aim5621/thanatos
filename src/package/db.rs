use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const DB_PATH: &str = "/var/lib/thanatos/db.json";

fn get_db_path() -> PathBuf {
    std::env::var("THANATOS_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DB_PATH))
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum InstallReason {
    Explicit,
    Dependency,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum InstallState {
    Installed {
        version: String,
        release: u32,
        files: Vec<String>,
        reason: InstallReason,
    },
    Pending,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub depends: Vec<String>,
    pub state: InstallState,
}

impl Package {
    pub fn new(name: &str) -> Self {
        Package {
            name: name.to_string(),
            depends: vec![],
            state: InstallState::Pending,
        }
    }

    pub fn is_installed(&self) -> bool {
        matches!(self.state, InstallState::Installed { .. })
    }

    pub fn version(&self) -> Option<&str> {
        match &self.state {
            InstallState::Installed { version, .. } => Some(version),
            InstallState::Pending => None,
        }
    }

    pub fn files(&self) -> Option<&[String]> {
        match &self.state {
            InstallState::Installed { files, .. } => Some(files),
            InstallState::Pending => None,
        }
    }

    pub fn reason(&self) -> Option<&InstallReason> {
        match &self.state {
            InstallState::Installed { reason, .. } => Some(reason),
            InstallState::Pending => None,
        }
    }

    pub fn mark_installed(
        &mut self,
        version: String,
        release: u32,
        files: Vec<String>,
        reason: InstallReason,
    ) {
        self.state = InstallState::Installed {
            version,
            release,
            files,
            reason,
        };
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct PackageDb {
    packages: BTreeMap<String, Package>,
}

impl PackageDb {
    pub fn load_from(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(PackageDb::default());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = get_db_path();
        if !path.exists() {
            return Ok(PackageDb::default());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = get_db_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn insert(&mut self, pkg: Package) {
        self.packages.insert(pkg.name.clone(), pkg);
    }

    pub fn remove(&mut self, name: &str) -> Option<Package> {
        self.packages.remove(name)
    }

    pub fn get(&self, name: &str) -> Option<&Package> {
        self.packages.get(name)
    }

    pub fn is_installed(&self, name: &str) -> bool {
        match self.packages.get(name) {
            Some(pkg) => pkg.is_installed(),
            None => false,
        }
    }

    pub fn all(&self) -> Vec<&Package> {
        self.packages.values().collect()
    }

    pub fn explicit(&self) -> Vec<&Package> {
        self.packages
            .values()
            .filter(|p| matches!(p.reason(), Some(InstallReason::Explicit)))
            .collect()
    }

    pub fn dependencies(&self) -> Vec<&Package> {
        self.packages
            .values()
            .filter(|p| matches!(p.reason(), Some(InstallReason::Dependency)))
            .collect()
    }

    pub fn orphans(&self) -> Vec<&Package> {
        self.packages
            .values()
            .filter(|pkg| matches!(pkg.reason(), Some(InstallReason::Dependency)))
            .filter(|pkg| {
                !self
                    .packages
                    .values()
                    .any(|other| other.depends.contains(&pkg.name))
            })
            .collect()
    }

    pub fn dependents_of(&self, name: &str) -> Vec<&Package> {
        self.packages
            .values()
            .filter(|pkg| pkg.depends.contains(&name.to_string()))
            .collect()
    }

    pub fn safe_to_remove(&self, name: &str) -> bool {
        self.dependents_of(name).is_empty()
    }
}
