use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn workspace_release_versions_are_synchronized() {
    let root_manifest = read_fixture("Cargo.toml");
    let warder_manifest = read_fixture("warder/Cargo.toml");
    let lock = read_fixture("Cargo.lock");
    let web_package = read_fixture("web/pkg/package.json");
    let dockerfile = read_fixture("Dockerfile");
    let mise = read_fixture(".mise.toml");

    let version = package_manifest_version(&root_manifest);
    assert_eq!(
        package_manifest_version(&warder_manifest),
        version,
        "Warder and the compiler must ship with one release version"
    );

    let dependency = format!("restrict_lang = {{ path = \"..\", version = \"{version}\" }}");
    assert!(
        warder_manifest.contains(&dependency),
        "Warder's compiler path dependency must require the release version"
    );

    for package in ["restrict_lang", "warder"] {
        assert_eq!(
            lock_package_version(&lock, package),
            version,
            "Cargo.lock should record the release version for {package}"
        );
    }

    assert!(
        web_package.contains(&format!(r#""version": "{version}""#)),
        "the checked-in wasm-pack package metadata must match the compiler"
    );
    assert!(
        dockerfile.contains(&format!("LABEL version=\"{version}\"")),
        "the development image label must match the release version"
    );
    assert!(
        mise.contains(&format!("restrict_lang:{version}")),
        "the development Docker tasks must use the release version"
    );
}

#[test]
fn current_version_has_curated_release_metadata() {
    let root_manifest = read_fixture("Cargo.toml");
    let version = package_manifest_version(&root_manifest);
    let tag = format!("v{version}");
    let notes_path = format!("docs/releases/{tag}.md");
    let notes = read_fixture(&notes_path);
    let changelog = read_fixture("CHANGELOG.md");
    let workflow = read_fixture(".github/workflows/release.yml");

    assert!(
        notes.starts_with(&format!("# Restrict Language {tag}\n")),
        "current release notes must start with the exact tag"
    );
    assert!(
        changelog.contains(&format!("## [{version}]")),
        "CHANGELOG must contain the current release version"
    );
    assert!(
        workflow.contains(&format!("default: {tag}")),
        "release dry runs should default to the current release tag"
    );
    assert!(
        workflow.contains("test -f \"docs/releases/v${cargo_version}.md\""),
        "release validation must require curated notes before building"
    );
    assert!(
        !workflow.contains("--generate-notes"),
        "tagged releases must not silently replace missing curated notes"
    );
}

#[test]
fn release_workflow_marks_only_semver_prereleases() {
    let workflow = read_fixture(".github/workflows/release.yml");

    for required in [
        "name: Publish GitHub release",
        "release_flags=(--verify-tag)",
        "if [[ \"$release_version\" == *-* ]]; then",
        "release_flags+=(--prerelease --latest=false)",
        "\"${release_flags[@]}\"",
        "--notes-file \"docs/releases/${{ github.ref_name }}.md\"",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow should include `{required}`"
        );
    }

    assert_eq!(
        workflow.matches("--prerelease").count(),
        1,
        "the prerelease flag must only be added by the SemVer prerelease branch"
    );
    assert_eq!(
        workflow.matches("--latest=false").count(),
        1,
        "only prereleases should be forced out of the latest channel"
    );

    let branch_start = workflow
        .find("if [[ \"$release_version\" == *-* ]]; then")
        .expect("SemVer prerelease branch should exist");
    let branch_end = workflow[branch_start..]
        .find("\n          fi")
        .map(|offset| branch_start + offset)
        .expect("SemVer prerelease branch should close");
    let prerelease_branch = &workflow[branch_start..branch_end];
    assert!(
        prerelease_branch.contains("--prerelease --latest=false"),
        "the sole prerelease flags must remain inside the SemVer branch"
    );
}

fn read_fixture(path: &str) -> String {
    fs::read_to_string(workspace_root().join(path))
        .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn package_manifest_version(manifest: &str) -> &str {
    let package = manifest
        .split_once("[package]")
        .map(|(_, rest)| rest)
        .expect("manifest should contain [package]")
        .split("\n[")
        .next()
        .expect("package table should have a body");
    quoted_assignment(package, "version").expect("package should declare a version")
}

fn lock_package_version<'a>(lock: &'a str, package_name: &str) -> &'a str {
    let package = lock
        .split("[[package]]")
        .skip(1)
        .find(|block| quoted_assignment(block, "name") == Some(package_name))
        .unwrap_or_else(|| panic!("Cargo.lock should contain {package_name}"));
    quoted_assignment(package, "version")
        .unwrap_or_else(|| panic!("Cargo.lock package {package_name} should have a version"))
}

fn quoted_assignment<'a>(section: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key} = \"");
    section.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix('"'))
    })
}
