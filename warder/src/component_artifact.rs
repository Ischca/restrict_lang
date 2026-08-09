use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

const COMPONENT_ARTIFACT_SCHEMA_VERSION: u32 = 1;
const COMPONENT_ARTIFACT_FORMAT: &str = "warder.component-artifact.v1";

/// Deterministic Warder metadata for one published WebAssembly Component.
///
/// The three hashes deliberately have different meanings. In particular,
/// `interface_sha256` is supplied by the compiler and remains independent
/// from the exact Component bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComponentArtifactManifest {
    pub(crate) schema: u32,
    pub(crate) format: String,
    pub(crate) package: String,
    pub(crate) world: String,
    pub(crate) abi: String,
    pub(crate) implementation_subset: String,
    pub(crate) interface_sha256: String,
    pub(crate) component_sha256: String,
    pub(crate) wit_sha256: String,
}

impl ComponentArtifactManifest {
    pub(crate) fn new(
        package: String,
        world: String,
        abi: String,
        implementation_subset: String,
        interface_sha256: String,
        component_wasm: &[u8],
        normalized_wit: &str,
    ) -> Result<Self> {
        validate_lowercase_sha256("compiler interface hash", &interface_sha256)?;
        Ok(Self {
            schema: COMPONENT_ARTIFACT_SCHEMA_VERSION,
            format: COMPONENT_ARTIFACT_FORMAT.to_string(),
            package,
            world,
            abi,
            implementation_subset,
            interface_sha256,
            component_sha256: sha256_hex(component_wasm),
            wit_sha256: sha256_hex(normalized_wit.as_bytes()),
        })
    }

    fn validate_shape(&self) -> Result<()> {
        if self.schema != COMPONENT_ARTIFACT_SCHEMA_VERSION {
            bail!(
                "Unsupported Component artifact metadata schema {}; expected {}",
                self.schema,
                COMPONENT_ARTIFACT_SCHEMA_VERSION
            );
        }
        if self.format != COMPONENT_ARTIFACT_FORMAT {
            bail!(
                "Unsupported Component artifact metadata format '{}'; expected '{}'",
                self.format,
                COMPONENT_ARTIFACT_FORMAT
            );
        }
        if self.package.is_empty()
            || self.world.is_empty()
            || self.abi.is_empty()
            || self.implementation_subset.is_empty()
        {
            bail!("Component artifact metadata identity fields must not be empty");
        }
        validate_lowercase_sha256("interface_sha256", &self.interface_sha256)?;
        validate_lowercase_sha256("component_sha256", &self.component_sha256)?;
        validate_lowercase_sha256("wit_sha256", &self.wit_sha256)?;
        Ok(())
    }
}

pub(crate) fn write_staged_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "Failed to create staged Component artifact {}",
                path.display()
            )
        })?;
    file.write_all(bytes).with_context(|| {
        format!(
            "Failed to write staged Component artifact {}",
            path.display()
        )
    })?;
    file.sync_all().with_context(|| {
        format!(
            "Failed to flush staged Component artifact {}",
            path.display()
        )
    })
}

pub(crate) fn write_staged_manifest(
    path: &Path,
    manifest: &ComponentArtifactManifest,
) -> Result<()> {
    manifest.validate_shape()?;
    let serialized = toml::to_string_pretty(manifest)
        .context("Failed to serialize Component artifact metadata")?;
    write_staged_file(path, serialized.as_bytes())
}

/// Re-open and validate the complete staged set before it enters Warder's
/// artifact transaction. This catches write corruption and keeps validation
/// on the exact bytes that will be renamed into the public output paths.
pub(crate) fn validate_staged_component_artifacts(
    component_path: &Path,
    wit_path: &Path,
    manifest_path: &Path,
    expected: &ComponentArtifactManifest,
) -> Result<ComponentArtifactManifest> {
    let component_wasm = read_regular_file(component_path, "Component Wasm")?;
    if !wasmparser::Parser::is_component(&component_wasm) {
        bail!(
            "Staged Component output is a Core Wasm module, not a WebAssembly Component: {}",
            component_path.display()
        );
    }
    wasmparser::Validator::new_with_features(
        wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
    )
    .validate_all(&component_wasm)
    .with_context(|| {
        format!(
            "Staged WebAssembly Component failed validation: {}",
            component_path.display()
        )
    })?;

    let wit_bytes = read_regular_file(wit_path, "normalized WIT")?;
    let normalized_wit = std::str::from_utf8(&wit_bytes)
        .with_context(|| format!("Staged WIT is not UTF-8: {}", wit_path.display()))?;
    let manifest_bytes = read_regular_file(manifest_path, "Component metadata")?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).with_context(|| {
        format!(
            "Staged Component metadata is not UTF-8: {}",
            manifest_path.display()
        )
    })?;
    let manifest: ComponentArtifactManifest = toml::from_str(manifest_text).with_context(|| {
        format!(
            "Failed to parse staged Component metadata {}",
            manifest_path.display()
        )
    })?;
    manifest.validate_shape()?;
    if &manifest != expected {
        bail!("Staged Component metadata does not match the compiler build result");
    }

    if sha256_hex(&component_wasm) != manifest.component_sha256 {
        bail!("Staged Component bytes do not match component_sha256 metadata");
    }
    if sha256_hex(&wit_bytes) != manifest.wit_sha256 {
        bail!("Staged WIT does not match wit_sha256 metadata");
    }

    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str("warder-generated-component.wit", normalized_wit)
        .context("Staged normalized WIT failed parsing or resolution")?;
    let wit_package = resolve.packages[package].name.to_string();
    if wit_package != manifest.package {
        bail!(
            "Staged normalized WIT package '{}' does not match metadata package '{}'",
            wit_package,
            manifest.package
        );
    }
    resolve
        .select_world(&[package], Some(&manifest.world))
        .with_context(|| {
            format!(
                "Staged normalized WIT does not define selected world '{}'",
                manifest.world
            )
        })?;

    Ok(manifest)
}

fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("Staged {label} is missing: {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("Staged {label} is not a regular file: {}", path.display());
    }
    std::fs::read(path).with_context(|| format!("Failed to read staged {label} {}", path.display()))
}

fn validate_lowercase_sha256(label: &str, value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        bail!("{label} must be 64 lowercase hexadecimal characters");
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn empty_component() -> Vec<u8> {
        wat::parse_str("(component)").unwrap()
    }

    fn sample_wit() -> &'static str {
        "package example:app@0.1.0;\n\nworld app {}\n"
    }

    #[test]
    fn staged_component_set_round_trips_and_hashes_exact_bytes() {
        let temp = TempDir::new().unwrap();
        let component_path = temp.path().join("app.component.wasm");
        let wit_path = temp.path().join("app.component.wit");
        let manifest_path = temp.path().join("app.component.toml");
        let component = empty_component();
        let manifest = ComponentArtifactManifest::new(
            "example:app@0.1.0".to_string(),
            "app".to_string(),
            "canonical-v1-sync".to_string(),
            "scalar-v1".to_string(),
            "a".repeat(64),
            &component,
            sample_wit(),
        )
        .unwrap();

        write_staged_file(&component_path, &component).unwrap();
        write_staged_file(&wit_path, sample_wit().as_bytes()).unwrap();
        write_staged_manifest(&manifest_path, &manifest).unwrap();

        assert_eq!(
            validate_staged_component_artifacts(
                &component_path,
                &wit_path,
                &manifest_path,
                &manifest,
            )
            .unwrap(),
            manifest
        );
    }

    #[test]
    fn staged_component_validation_rejects_core_wasm_and_tampering() {
        let temp = TempDir::new().unwrap();
        let component_path = temp.path().join("app.component.wasm");
        let wit_path = temp.path().join("app.component.wit");
        let manifest_path = temp.path().join("app.component.toml");
        let component = empty_component();
        let manifest = ComponentArtifactManifest::new(
            "example:app@0.1.0".to_string(),
            "app".to_string(),
            "canonical-v1-sync".to_string(),
            "scalar-v1".to_string(),
            "b".repeat(64),
            &component,
            sample_wit(),
        )
        .unwrap();

        write_staged_file(&component_path, &wat::parse_str("(module)").unwrap()).unwrap();
        write_staged_file(&wit_path, sample_wit().as_bytes()).unwrap();
        write_staged_manifest(&manifest_path, &manifest).unwrap();
        assert!(validate_staged_component_artifacts(
            &component_path,
            &wit_path,
            &manifest_path,
            &manifest,
        )
        .unwrap_err()
        .to_string()
        .contains("Core Wasm module"));

        std::fs::write(&component_path, &component).unwrap();
        std::fs::write(&wit_path, "package example:app@0.1.0;\n\nworld other {}\n").unwrap();
        assert!(validate_staged_component_artifacts(
            &component_path,
            &wit_path,
            &manifest_path,
            &manifest,
        )
        .unwrap_err()
        .to_string()
        .contains("wit_sha256"));
    }

    #[test]
    fn metadata_is_deterministic_and_rejects_noncanonical_interface_hash() {
        let component = empty_component();
        let first = ComponentArtifactManifest::new(
            "example:app@0.1.0".to_string(),
            "app".to_string(),
            "canonical-v1-sync".to_string(),
            "scalar-v1".to_string(),
            "c".repeat(64),
            &component,
            sample_wit(),
        )
        .unwrap();
        let second = first.clone();
        assert_eq!(
            toml::to_string_pretty(&first).unwrap(),
            toml::to_string_pretty(&second).unwrap()
        );

        assert!(ComponentArtifactManifest::new(
            "example:app@0.1.0".to_string(),
            "app".to_string(),
            "canonical-v1-sync".to_string(),
            "scalar-v1".to_string(),
            "A".repeat(64),
            &component,
            sample_wit(),
        )
        .is_err());
    }
}
