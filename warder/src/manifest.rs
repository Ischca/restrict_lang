use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    pub build: Build,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub edition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Local {
        path: String,
    },
    Git {
        git: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
    },
    Foreign {
        wasm: String,
        wit: String,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_optimize")]
    pub optimize: bool,
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            target: default_target(),
            optimize: default_optimize(),
            output: default_output(),
        }
    }
}

fn default_target() -> String {
    "wasm32".to_string()
}

fn default_optimize() -> bool {
    true
}

fn default_output() -> String {
    "dist/".to_string()
}

impl Manifest {
    pub fn new(name: &str) -> Self {
        Self {
            package: Package {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                entry: "src/main.rl".to_string(),
                edition: "2025".to_string(),
                authors: None,
                description: None,
            },
            dependencies: BTreeMap::new(),
            build: Build::default(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest from {:?}", path))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse manifest from {:?}", path))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize manifest")?;
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    anyhow::bail!(
                        "Refusing to replace non-regular package manifest {}",
                        path.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect package manifest {}", path.display())
                });
            }
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut staged = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("Failed to stage manifest next to {}", path.display()))?;
        staged
            .write_all(content.as_bytes())
            .with_context(|| format!("Failed to write staged manifest for {}", path.display()))?;
        staged
            .as_file_mut()
            .sync_all()
            .with_context(|| format!("Failed to flush staged manifest for {}", path.display()))?;
        staged.persist(path).map(|_| ()).with_context(|| {
            format!(
                "Failed to atomically replace manifest at {}",
                path.display()
            )
        })
    }

    pub fn add_dependency(&mut self, name: String, dep: Dependency) {
        self.dependencies.insert(name, dep);
    }

    pub fn remove_dependency(&mut self, name: &str) -> Option<Dependency> {
        self.dependencies.remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn manifest_serialization_orders_dependencies_by_alias() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.rl.toml");
        let mut manifest = Manifest::new("app");
        manifest.add_dependency(
            "zeta".to_string(),
            Dependency::Local {
                path: "../zeta".to_string(),
            },
        );
        manifest.add_dependency(
            "alpha".to_string(),
            Dependency::Local {
                path: "../alpha".to_string(),
            },
        );

        manifest.save(&path).unwrap();
        let serialized = std::fs::read_to_string(path).unwrap();

        assert!(
            serialized.find("[dependencies.alpha]").unwrap()
                < serialized.find("[dependencies.zeta]").unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn manifest_save_does_not_follow_a_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let outside = temp.path().join("outside.toml");
        let path = temp.path().join("package.rl.toml");
        std::fs::write(&outside, "keep\n").unwrap();
        symlink(&outside, &path).unwrap();

        let error = Manifest::new("app").save(&path).unwrap_err();

        assert!(error.to_string().contains("non-regular package manifest"));
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep\n");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CageManifest {
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub freeze_ts: u64,
    pub abi_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wit_exports: Option<Vec<String>>,
}

impl CageManifest {
    pub fn new(name: String, version: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        Self {
            name,
            version,
            sha256: String::new(),
            freeze_ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            abi_hash: String::new(),
            wit_exports: None,
        }
    }
}
