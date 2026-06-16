use crate::manifest::CageManifest;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

const ABI_HASH_FORMAT_VERSION: &str = "warder.cage.abi-content.v0.0.1";

pub struct Cage {
    pub manifest: CageManifest,
    pub wasm_bytes: Vec<u8>,
    pub wit_files: Vec<(String, String)>, // (filename, content)
}

impl Cage {
    pub fn new(name: String, version: String, wasm_bytes: Vec<u8>) -> Self {
        let mut manifest = CageManifest::new(name, version);

        // Calculate SHA256 of WASM
        let mut hasher = Sha256::new();
        hasher.update(&wasm_bytes);
        manifest.sha256 = hex::encode(hasher.finalize());

        Self {
            manifest,
            wasm_bytes,
            wit_files: Vec::new(),
        }
    }

    pub fn add_wit(&mut self, filename: String, content: String) {
        // Update manifest with WIT exports
        if self.manifest.wit_exports.is_none() {
            self.manifest.wit_exports = Some(Vec::new());
        }
        if let Some(exports) = &mut self.manifest.wit_exports {
            exports.push(filename.clone());
        }

        self.wit_files.push((filename, content));
    }

    pub fn calculate_abi_hash(&mut self) -> Result<()> {
        let mut wit_entries: Vec<(&str, &str)> = self
            .wit_files
            .iter()
            .map(|(filename, content)| (filename.as_str(), content.as_str()))
            .collect();
        wit_entries.sort_unstable();

        self.manifest.wit_exports = if wit_entries.is_empty() {
            None
        } else {
            Some(
                wit_entries
                    .iter()
                    .map(|(filename, _)| (*filename).to_string())
                    .collect(),
            )
        };

        let mut hasher = Sha256::new();
        update_hash_field(&mut hasher, "format", ABI_HASH_FORMAT_VERSION.as_bytes());
        update_hash_field(&mut hasher, "module.wasm", &self.wasm_bytes);
        update_hash_u64(&mut hasher, "wit.count", wit_entries.len() as u64);

        for (filename, content) in wit_entries {
            update_hash_field(&mut hasher, "wit.filename", filename.as_bytes());
            update_hash_field(&mut hasher, "wit.content", content.as_bytes());
        }
        self.manifest.abi_hash = hex::encode(hasher.finalize());
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .with_context(|| format!("Failed to create cage file at {:?}", path))?;
        let mut zip = ZipWriter::new(file);
        // zip 8 made FileOptions generic over its extension type; a basic
        // archive needs no extra options, so pin the unit extension.
        let options = FileOptions::<'_, ()>::default();

        // Write manifest
        let manifest_toml = toml::to_string(&self.manifest)?;
        zip.start_file("manifest.toml", options)?;
        zip.write_all(manifest_toml.as_bytes())?;

        // Write WASM module
        zip.start_file("module.wasm", options)?;
        zip.write_all(&self.wasm_bytes)?;

        // Write WIT files
        if !self.wit_files.is_empty() {
            for (filename, content) in &self.wit_files {
                zip.start_file(format!("wit/{}", filename), options)?;
                zip.write_all(content.as_bytes())?;
            }
        }

        zip.finish()?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("Failed to open cage file at {:?}", path))?;
        let mut archive = ZipArchive::new(file)?;

        // Read manifest
        let manifest_file = archive.by_name("manifest.toml")?;
        let manifest: CageManifest = {
            let mut content = String::new();
            let mut reader = manifest_file;
            reader.read_to_string(&mut content)?;
            toml::from_str(&content)?
        };

        // Read WASM module
        let wasm_bytes = {
            let mut file = archive.by_name("module.wasm")?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            bytes
        };

        // Read WIT files
        let mut wit_files = Vec::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name.starts_with("wit/") && name.ends_with(".wit") {
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                let filename = name.strip_prefix("wit/").unwrap().to_string();
                wit_files.push((filename, content));
            }
        }

        Ok(Self {
            manifest,
            wasm_bytes,
            wit_files,
        })
    }

    pub fn extract(&self, output_dir: &Path) -> Result<()> {
        fs::create_dir_all(output_dir)?;

        // Write WASM module
        let wasm_path = output_dir.join("module.wasm");
        fs::write(&wasm_path, &self.wasm_bytes)?;

        // Write WIT files
        if !self.wit_files.is_empty() {
            let wit_dir = output_dir.join("wit");
            fs::create_dir_all(&wit_dir)?;
            for (filename, content) in &self.wit_files {
                fs::write(wit_dir.join(filename), content)?;
            }
        }

        // Write manifest
        let manifest_path = output_dir.join("manifest.toml");
        let manifest_toml = toml::to_string(&self.manifest)?;
        fs::write(manifest_path, manifest_toml)?;

        Ok(())
    }
}

fn update_hash_field(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn update_hash_u64(hasher: &mut Sha256, label: &str, value: u64) {
    update_hash_field(hasher, label, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cage(wasm_bytes: Vec<u8>) -> Cage {
        Cage::new("demo".to_string(), "0.0.1".to_string(), wasm_bytes)
    }

    #[test]
    fn abi_hash_is_stable_for_wit_insertion_order() {
        let wasm_bytes = b"\0asm\x01\0\0\0".to_vec();

        let mut first = sample_cage(wasm_bytes.clone());
        first.add_wit("zeta.wit".to_string(), "interface zeta {}".to_string());
        first.add_wit("alpha.wit".to_string(), "interface alpha {}".to_string());
        first.calculate_abi_hash().unwrap();

        let mut second = sample_cage(wasm_bytes);
        second.add_wit("alpha.wit".to_string(), "interface alpha {}".to_string());
        second.add_wit("zeta.wit".to_string(), "interface zeta {}".to_string());
        second.calculate_abi_hash().unwrap();

        assert_eq!(first.manifest.abi_hash, second.manifest.abi_hash);
        assert_eq!(first.manifest.abi_hash.len(), 64);
        assert_eq!(
            first.manifest.wit_exports,
            Some(vec!["alpha.wit".to_string(), "zeta.wit".to_string()])
        );
    }

    #[test]
    fn abi_hash_distinguishes_canonical_content() {
        let mut base = sample_cage(b"\0asm\x01\0\0\0".to_vec());
        base.add_wit("world.wit".to_string(), "interface world {}".to_string());
        base.calculate_abi_hash().unwrap();

        let mut changed_wasm = sample_cage(b"\0asm\x01\0\0\x01".to_vec());
        changed_wasm.add_wit("world.wit".to_string(), "interface world {}".to_string());
        changed_wasm.calculate_abi_hash().unwrap();

        let mut changed_wit_name = sample_cage(b"\0asm\x01\0\0\0".to_vec());
        changed_wit_name.add_wit("other.wit".to_string(), "interface world {}".to_string());
        changed_wit_name.calculate_abi_hash().unwrap();

        let mut changed_wit_content = sample_cage(b"\0asm\x01\0\0\0".to_vec());
        changed_wit_content.add_wit("world.wit".to_string(), "interface world-v2 {}".to_string());
        changed_wit_content.calculate_abi_hash().unwrap();

        assert_ne!(base.manifest.abi_hash, changed_wasm.manifest.abi_hash);
        assert_ne!(base.manifest.abi_hash, changed_wit_name.manifest.abi_hash);
        assert_ne!(
            base.manifest.abi_hash,
            changed_wit_content.manifest.abi_hash
        );
    }
}
