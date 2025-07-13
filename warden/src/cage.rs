use anyhow::{Result, Context};
use std::fs;
use std::path::Path;
use std::fs::File;
use std::io::{Read, Write};
use zip::{ZipWriter, ZipArchive, write::FileOptions};
use sha2::{Sha256, Digest};
use crate::manifest::CageManifest;

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
        // TODO: Implement proper ABI hash calculation
        // For now, use a simple hash of WASM + WIT content
        let mut hasher = Sha256::new();
        hasher.update(&self.wasm_bytes);
        for (_, content) in &self.wit_files {
            hasher.update(content.as_bytes());
        }
        self.manifest.abi_hash = hex::encode(hasher.finalize());
        Ok(())
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .with_context(|| format!("Failed to create cage file at {:?}", path))?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        
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
        let file = File::open(path)
            .with_context(|| format!("Failed to open cage file at {:?}", path))?;
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