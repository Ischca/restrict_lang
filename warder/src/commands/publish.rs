use super::{find_project_root, load_manifest, print_info, print_warning};
use crate::cage::Cage;
use anyhow::{bail, Result};

pub async fn publish_package(registry: Option<String>) -> Result<()> {
    let root = find_project_root()?;
    let manifest = load_manifest()?;

    // Default registry
    let registry_url = registry.unwrap_or_else(|| "https://wardhub.restrict-lang.org".to_string());

    // Build in release mode first
    print_info("Running publish preflight build...");
    super::build::build_project(true, false, false, true, true).await?;

    // Find the built cage
    let build_dir = root.join(&manifest.build.output);
    let cage_name = format!("{}-{}.rgc", manifest.package.name, manifest.package.version);
    let cage_path = build_dir.join(&cage_name);

    if !cage_path.exists() {
        bail!("Built cage not found. Build failed?");
    }

    // Load cage to verify
    let cage = Cage::load(&cage_path)?;

    // Verify package metadata
    if cage.manifest.name != manifest.package.name {
        bail!("Package name mismatch");
    }

    if cage.manifest.version != manifest.package.version {
        bail!("Package version mismatch");
    }

    print_info(&format!(
        "Publish preflight complete for {} v{}",
        manifest.package.name, manifest.package.version
    ));

    // TODO: Implement actual publishing
    // This would:
    // 1. Authenticate with the registry
    // 2. Sign the cage with sigstore
    // 3. Upload the cage
    // 4. Update registry index

    print_warning(&format!(
        "Registry publishing is experimental and out-of-scope for v0.0.1; no package was uploaded to {}",
        registry_url
    ));
    print_info("Registry publish support is intentionally not implemented in this release.");
    print_info("Validated package metadata only; use the generated cage for local evaluation.");

    Ok(())
}
