use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone)]
pub struct Registry {
    url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub dependencies: HashMap<String, String>,
    pub cage_url: String,
    pub sha256: String,
    pub abi_hash: String,
    pub published_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageMetadata>,
    pub total: usize,
}

impl Registry {
    pub fn new(url: &str) -> Result<Self> {
        let url = Url::parse(url)
            .with_context(|| format!("Invalid registry URL: {}", url))?;
        
        Ok(Self {
            url,
            client: reqwest::Client::new(),
        })
    }
    
    pub async fn search(&self, query: &str) -> Result<SearchResult> {
        let search_url = self.url.join("/api/v1/search")?;
        
        let response = self.client
            .get(search_url)
            .query(&[("q", query)])
            .send()
            .await
            .context("Failed to search registry")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Registry search failed: {}", response.status());
        }
        
        response.json()
            .await
            .context("Failed to parse search results")
    }
    
    pub async fn get_package(&self, name: &str, version: &str) -> Result<PackageMetadata> {
        let package_url = self.url.join(&format!("/api/v1/packages/{}/{}", name, version))?;
        
        let response = self.client
            .get(package_url)
            .send()
            .await
            .context("Failed to fetch package metadata")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Package not found: {} v{}", name, version);
        }
        
        response.json()
            .await
            .context("Failed to parse package metadata")
    }
    
    pub async fn download_cage(&self, metadata: &PackageMetadata) -> Result<Vec<u8>> {
        let response = self.client
            .get(&metadata.cage_url)
            .send()
            .await
            .context("Failed to download cage")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to download cage: {}", response.status());
        }
        
        let bytes = response.bytes()
            .await
            .context("Failed to read cage data")?;
        
        // Verify SHA256
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = hex::encode(hasher.finalize());
        
        if hash != metadata.sha256 {
            anyhow::bail!("Cage integrity check failed: SHA256 mismatch");
        }
        
        Ok(bytes.to_vec())
    }
    
    pub async fn publish(&self, _cage_data: &[u8], _metadata: &PackageMetadata, _token: &str) -> Result<()> {
        // TODO: Implement publishing
        anyhow::bail!("Publishing not implemented yet")
    }
}