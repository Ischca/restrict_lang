use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::Path;

pub const WARDER_MANIFEST_FILENAME: &str = "warder.toml";
pub const WARDER_LOCK_FILENAME: &str = "warder.lock";
pub const LEGACY_MANIFEST_FILENAME: &str = "package.rl.toml";
pub const LEGACY_LOCK_FILENAME: &str = "restrict-lock.toml";
pub const MANIFEST_VERSION: u32 = 1;

pub const COMPONENT_SCHEMA_VERSION: u32 =
    restrict_lang::component::COMPONENT_SELECTION_SCHEMA_VERSION;
pub const COMPONENT_SCHEMA_VERSION_V2: u32 =
    restrict_lang::component_selection_v2::COMPONENT_SELECTION_SCHEMA_V2;
pub const COMPONENT_ABI_PROFILE: &str = restrict_lang::component::COMPONENT_ABI_PROFILE;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(rename = "manifest-version", default = "default_manifest_version")]
    pub manifest_version: u32,
    pub package: Package,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
    #[serde(default, skip_serializing_if = "is_default_build")]
    pub build: Build,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<ComponentConfig>,
}

/// Explicit package-level selection for the synchronous Component interface.
/// Source-level `pub` remains necessary but is not sufficient: only manifest
/// declarations selected by schema v1/v2 enter the published Component ABI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentConfig {
    pub schema: u32,
    pub package: String,
    pub world: String,
    pub abi: String,
    #[serde(default)]
    pub exports: Vec<ComponentExport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ComponentResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callables: Vec<ComponentCallable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentExport {
    pub interface: String,
    pub source: String,
    pub name: String,
    #[serde(
        rename = "type-args",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub type_args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentResource {
    pub interface: String,
    pub source: String,
    pub name: String,
    pub destructor: String,
    #[serde(rename = "destructor-visibility")]
    pub destructor_visibility: ComponentSourceVisibility,
    #[serde(default)]
    pub methods: Vec<ComponentResourceMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentSourceVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentResourceMethod {
    pub source: String,
    pub name: String,
    pub receiver: ComponentResourceReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentResourceReceiver {
    Own,
    Borrow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCallable {
    pub interface: String,
    pub name: String,
    #[serde(default)]
    pub params: Vec<ComponentCallableParam>,
    pub result: String,
    pub multiplicity: ComponentCallableMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCallableParam {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentCallableMultiplicity {
    Once,
    Many,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default = "default_entry", skip_serializing_if = "is_default_entry")]
    pub entry: String,
    #[serde(
        default = "default_edition",
        skip_serializing_if = "is_default_edition"
    )]
    pub edition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

fn default_entry() -> String {
    "src/main.rl".to_string()
}

fn is_default_entry(value: &str) -> bool {
    value == "src/main.rl"
}

fn default_edition() -> String {
    "2025".to_string()
}

fn is_default_edition(value: &str) -> bool {
    value == "2025"
}

fn default_manifest_version() -> u32 {
    MANIFEST_VERSION
}

fn default_optimize() -> bool {
    true
}

fn default_output() -> String {
    "dist/".to_string()
}

fn is_default_build(build: &Build) -> bool {
    build == &Build::default()
}

impl Manifest {
    pub fn new(name: &str) -> Self {
        Self {
            manifest_version: MANIFEST_VERSION,
            package: Package {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                entry: default_entry(),
                edition: default_edition(),
                authors: None,
                description: None,
            },
            dependencies: BTreeMap::new(),
            build: Build::default(),
            component: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest from {:?}", path))?;
        let manifest: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse manifest from {:?}", path))?;
        manifest
            .validate()
            .with_context(|| format!("Invalid manifest at {}", path.display()))?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.manifest_version != MANIFEST_VERSION {
            anyhow::bail!(
                "Unsupported Warder manifest version {}; expected {}",
                self.manifest_version,
                MANIFEST_VERSION
            );
        }
        if let Some(component) = &self.component {
            component.validate(&self.package)?;
        }
        Ok(())
    }

    /// Translate Warder's validated TOML schema into the compiler's neutral
    /// selection model without making the compiler depend on manifest types.
    pub fn compiler_component_selection(
        &self,
    ) -> Result<Option<restrict_lang::component::ComponentSelection>> {
        self.validate()?;
        let Some(component) = self.component.as_ref() else {
            return Ok(None);
        };
        if component.schema != COMPONENT_SCHEMA_VERSION {
            anyhow::bail!(
                "component schema {} cannot be represented by the legacy schema-v1 selection; use the compiler Component contract",
                component.schema
            );
        }
        Ok(Some(restrict_lang::component::ComponentSelection::v1(
            component.package.clone(),
            component.world.clone(),
            component.abi.clone(),
            component
                .exports
                .iter()
                .map(|export| {
                    restrict_lang::component::ComponentExportSelection::new(
                        export.interface.clone(),
                        export.source.clone(),
                        export.name.clone(),
                    )
                })
                .collect(),
        )))
    }

    /// Translate either supported manifest schema to the deterministic,
    /// compiler-neutral declaration contract used by Component publication.
    pub fn compiler_component_contract(
        &self,
    ) -> Result<Option<restrict_lang::component_selection_v2::ComponentSelectionContract>> {
        self.validate()?;
        self.component
            .as_ref()
            .map(ComponentConfig::compiler_contract)
            .transpose()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()
            .with_context(|| format!("Refusing to save invalid manifest at {}", path.display()))?;
        let content = toml::to_string_pretty(self).context("Failed to serialize manifest")?;
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    anyhow::bail!(
                        "Refusing to replace non-regular Warder manifest {}",
                        path.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect Warder manifest {}", path.display())
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

impl ComponentConfig {
    fn validate(&self, source_package: &Package) -> Result<()> {
        if !matches!(
            self.schema,
            COMPONENT_SCHEMA_VERSION | COMPONENT_SCHEMA_VERSION_V2
        ) {
            anyhow::bail!(
                "Unsupported component schema {}; expected {} or {}",
                self.schema,
                COMPONENT_SCHEMA_VERSION,
                COMPONENT_SCHEMA_VERSION_V2
            );
        }
        if self.abi != COMPONENT_ABI_PROFILE {
            anyhow::bail!(
                "Unsupported component ABI profile '{}'; expected '{}'",
                self.abi,
                COMPONENT_ABI_PROFILE
            );
        }

        let component_package = validate_wit_package(&self.package)?;
        if component_package.name != source_package.name {
            anyhow::bail!(
                "component.package name '{}' must match package.name '{}' for component schema {}",
                component_package.name,
                source_package.name,
                self.schema
            );
        }
        if component_package.version != source_package.version {
            anyhow::bail!(
                "component.package version '{}' must match package.version '{}' for component schema {}",
                component_package.version,
                source_package.version,
                self.schema
            );
        }
        validate_wit_identifier("component.world", &self.world)?;

        if self.schema == COMPONENT_SCHEMA_VERSION && self.exports.is_empty() {
            anyhow::bail!("component.exports must contain at least one explicit export");
        }
        if self.schema == COMPONENT_SCHEMA_VERSION
            && self
                .exports
                .iter()
                .any(|export| !export.type_args.is_empty())
        {
            anyhow::bail!(
                "component schema 1 does not accept component.exports.type-args; use schema 2"
            );
        }
        if self.schema == COMPONENT_SCHEMA_VERSION && !self.resources.is_empty() {
            anyhow::bail!("component schema 1 does not accept component.resources; use schema 2");
        }
        if self.schema == COMPONENT_SCHEMA_VERSION && !self.callables.is_empty() {
            anyhow::bail!("component schema 1 does not accept component.callables; use schema 2");
        }
        if self.schema == COMPONENT_SCHEMA_VERSION_V2
            && self.exports.is_empty()
            && self.resources.is_empty()
            && self.callables.is_empty()
        {
            anyhow::bail!(
                "component schema 2 requires at least one explicit export, resource, or callable"
            );
        }

        let mut sources = HashSet::new();
        let mut wit_exports = HashSet::new();
        for (index, export) in self.exports.iter().enumerate() {
            let field = format!("component.exports[{index}]");
            validate_wit_identifier(&format!("{field}.interface"), &export.interface)?;
            validate_wit_identifier(&format!("{field}.name"), &export.name)?;

            if !restrict_lang::lexer::is_source_identifier(&export.source) {
                anyhow::bail!(
                    "Invalid {field}.source '{}': expected one non-keyword root-module Restrict identifier",
                    export.source
                );
            }
            if self.schema == COMPONENT_SCHEMA_VERSION && !sources.insert(export.source.as_str()) {
                anyhow::bail!(
                    "Duplicate component export source '{}': each root-module declaration may be selected only once",
                    export.source
                );
            }
            if !wit_exports.insert((export.interface.as_str(), export.name.as_str())) {
                anyhow::bail!(
                    "Duplicate component WIT export '{}.{}'",
                    export.interface,
                    export.name
                );
            }
        }

        self.compiler_contract()?;
        Ok(())
    }

    fn compiler_contract(
        &self,
    ) -> Result<restrict_lang::component_selection_v2::ComponentSelectionContract> {
        use restrict_lang::component_selection_v2 as contract;

        let exports = self
            .exports
            .iter()
            .enumerate()
            .map(|(index, export)| {
                let type_arguments = export
                    .type_args
                    .iter()
                    .map(|(parameter, source)| {
                        let ty = contract::parse_closed_component_type(source).with_context(|| {
                            format!(
                                "Invalid component.exports[{index}].type-args.{parameter} '{source}'"
                            )
                        })?;
                        Ok(contract::ComponentTypeArgumentContract {
                            parameter: parameter.clone(),
                            ty,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(contract::ComponentFunctionExportContract {
                    interface: export.interface.clone(),
                    source_function: export.source.clone(),
                    external_name: export.name.clone(),
                    type_arguments,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let resources = self
            .resources
            .iter()
            .map(|resource| contract::ComponentResourceContract {
                interface: resource.interface.clone(),
                source_record: resource.source.clone(),
                external_name: resource.name.clone(),
                destructor: contract::ComponentDestructorContract {
                    source_function: resource.destructor.clone(),
                    visibility: match resource.destructor_visibility {
                        ComponentSourceVisibility::Private => {
                            contract::ComponentSourceVisibility::Private
                        }
                        ComponentSourceVisibility::Public => {
                            contract::ComponentSourceVisibility::Public
                        }
                    },
                },
                methods: resource
                    .methods
                    .iter()
                    .map(|method| contract::ComponentResourceMethodContract {
                        source_function: method.source.clone(),
                        external_name: method.name.clone(),
                        receiver: match method.receiver {
                            ComponentResourceReceiver::Own => {
                                contract::ComponentResourceReceiver::Own
                            }
                            ComponentResourceReceiver::Borrow => {
                                contract::ComponentResourceReceiver::Borrow
                            }
                        },
                    })
                    .collect(),
            })
            .collect();

        let callables = self
            .callables
            .iter()
            .enumerate()
            .map(|(callable_index, callable)| {
                let params = callable
                    .params
                    .iter()
                    .enumerate()
                    .map(|(param_index, param)| {
                        let ty = contract::parse_closed_component_type(&param.ty).with_context(
                            || {
                                format!(
                                    "Invalid component.callables[{callable_index}].params[{param_index}].type '{}'",
                                    param.ty
                                )
                            },
                        )?;
                        Ok(contract::ComponentCallableParamContract {
                            name: param.name.clone(),
                            ty,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let result = contract::parse_closed_component_type(&callable.result).with_context(
                    || {
                        format!(
                            "Invalid component.callables[{callable_index}].result '{}'",
                            callable.result
                        )
                    },
                )?;
                Ok(contract::ComponentCallableContract {
                    interface: callable.interface.clone(),
                    external_name: callable.name.clone(),
                    params,
                    result,
                    multiplicity: match callable.multiplicity {
                        ComponentCallableMultiplicity::Once => {
                            contract::ComponentCallableMultiplicity::Once
                        }
                        ComponentCallableMultiplicity::Many => {
                            contract::ComponentCallableMultiplicity::Many
                        }
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;

        contract::ComponentSelectionContract::try_new(
            self.schema,
            self.package.clone(),
            self.world.clone(),
            self.abi.clone(),
            exports,
            resources,
            callables,
        )
        .map_err(anyhow::Error::new)
    }
}

struct WitPackageIdentity<'a> {
    name: &'a str,
    version: &'a str,
}

fn validate_wit_package(package: &str) -> Result<WitPackageIdentity<'_>> {
    let Some((qualified_name, version)) = package.rsplit_once('@') else {
        anyhow::bail!(
            "Invalid component.package '{}': expected namespace:name@semver",
            package
        );
    };
    if qualified_name.contains('@') || version.is_empty() {
        anyhow::bail!(
            "Invalid component.package '{}': expected exactly one version suffix as namespace:name@semver",
            package
        );
    }

    let mut name_parts = qualified_name.split(':');
    let namespace = name_parts.next().unwrap_or_default();
    let name = name_parts.next().unwrap_or_default();
    if namespace.is_empty() || name.is_empty() || name_parts.next().is_some() {
        anyhow::bail!(
            "Invalid component.package '{}': expected namespace:name@semver",
            package
        );
    }

    validate_wit_identifier("component.package namespace", namespace)?;
    validate_wit_identifier("component.package name", name)?;
    semver::Version::parse(version).with_context(|| {
        format!(
            "Invalid component.package version '{}' in '{}': expected semantic versioning",
            version, package
        )
    })?;
    Ok(WitPackageIdentity { name, version })
}

fn validate_wit_identifier(field: &str, value: &str) -> Result<()> {
    if !is_conservative_wit_identifier(value) {
        anyhow::bail!(
            "Invalid {field} '{}': expected a lowercase kebab-case WIT identifier",
            value
        );
    }
    Ok(())
}

/// Warder deliberately accepts a conservative subset of WIT identifiers.
/// Escaped identifiers and keywords can be added with an explicit schema
/// revision rather than being normalized implicitly.
fn is_conservative_wit_identifier(value: &str) -> bool {
    restrict_lang::component::is_component_schema_v1_wit_identifier(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const VALID_COMPONENT: &str = r#"
[component]
schema = 1
package = "example:calculator@0.1.0"
world = "calculator"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "add"
name = "add"
"#;

    const VALID_COMPONENT_V2: &str = r#"
[component]
schema = 2
package = "example:calculator@0.1.0"
world = "calculator"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "identity"
name = "identity-string"
type-args = { T = "String" }

[[component.resources]]
interface = "api"
source = "FileHandle"
name = "file-handle"
destructor = "close_file"
destructor-visibility = "private"
methods = [
  { source = "read_file", name = "read", receiver = "borrow" },
  { source = "finish_file", name = "finish", receiver = "own" },
]

[[component.callables]]
interface = "api"
name = "transform"
params = [{ name = "value", type = "List<String>" }]
result = "Result<String, TransformError>"
multiplicity = "once"
"#;

    fn load_manifest_text(extra: &str) -> Result<Manifest> {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
        let source = format!(
            r#"
[package]
name = "calculator"
version = "0.1.0"
entry = "src/main.rl"
edition = "2025"
{extra}
"#
        );
        std::fs::write(&path, source).unwrap();
        Manifest::load(&path)
    }

    fn assert_invalid_component(extra: &str, expected: &str) {
        let error = load_manifest_text(extra).expect_err("component config should be rejected");
        let message = format!("{error:#}");
        assert!(
            message.contains(expected),
            "error should contain {expected:?}, got: {message}"
        );
    }

    #[test]
    fn manifest_serialization_orders_dependencies_by_alias() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
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

    #[test]
    fn manifest_new_has_no_component_selection() {
        let manifest = Manifest::new("app");
        assert_eq!(manifest.manifest_version, MANIFEST_VERSION);
        assert_eq!(manifest.component, None);

        let serialized = toml::to_string_pretty(&manifest).unwrap();
        assert!(serialized.contains("manifest-version = 1"));
        assert!(!serialized.contains("entry ="));
        assert!(!serialized.contains("edition ="));
        assert!(!serialized.contains("[dependencies]"));
        assert!(!serialized.contains("[build]"));
        assert!(!serialized.contains("[component]"));
    }

    #[test]
    fn manifest_loads_conventional_package_and_build_defaults() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
        std::fs::write(
            &path,
            r#"manifest-version = 1

[package]
name = "app"
version = "0.1.0"
"#,
        )
        .unwrap();

        let manifest = Manifest::load(&path).unwrap();
        assert_eq!(manifest.package.entry, "src/main.rl");
        assert_eq!(manifest.package.edition, "2025");
        assert!(manifest.dependencies.is_empty());
        assert_eq!(manifest.build, Build::default());
    }

    #[test]
    fn manifest_version_defaults_on_read_but_rejects_unsupported_versions() {
        let manifest = load_manifest_text("").unwrap();
        assert_eq!(manifest.manifest_version, MANIFEST_VERSION);

        let temp = TempDir::new().unwrap();
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
        std::fs::write(
            &path,
            r#"manifest-version = 2

[package]
name = "app"
version = "0.1.0"
entry = "src/main.rl"
edition = "2025"
"#,
        )
        .unwrap();

        let error = Manifest::load(&path).unwrap_err();
        assert!(
            format!("{error:#}").contains("Unsupported Warder manifest version 2"),
            "{error:#}"
        );
    }

    #[test]
    fn manifest_rejects_unknown_top_level_fields() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
        std::fs::write(
            &path,
            r#"manifest-version = 1
unknown-setting = true

[package]
name = "app"
version = "0.1.0"
entry = "src/main.rl"
edition = "2025"
"#,
        )
        .unwrap();

        let error = Manifest::load(&path).unwrap_err();
        assert!(format!("{error:#}").contains("unknown field `unknown-setting`"));
    }

    #[test]
    fn manifest_rejects_unknown_nested_fields() {
        let package_error = format!(
            "{:#}",
            load_manifest_text("unknown-setting = true").unwrap_err()
        );
        assert!(package_error.contains("unknown field `unknown-setting`"));

        let build_error = format!(
            "{:#}",
            load_manifest_text(
                r#"
[build]
target = "wasm32"
optimize = true
output = "dist/"
unknown-setting = true
"#,
            )
            .unwrap_err()
        );
        assert!(build_error.contains("unknown field `unknown-setting`"));
    }

    #[test]
    fn manifest_rejects_ambiguous_dependency_source_tables() {
        let error = format!(
            "{:#}",
            load_manifest_text(
                r#"
[dependencies.local_utils]
path = "../local-utils"
git = "https://example.test/local-utils.git"
"#,
            )
            .unwrap_err()
        );

        assert!(
            error.contains("did not match any variant of untagged enum Dependency"),
            "{error}"
        );
    }

    #[test]
    fn manifest_loads_versioned_component_selection() {
        let manifest = load_manifest_text(VALID_COMPONENT).unwrap();
        let serialized = toml::to_string_pretty(&manifest).unwrap();
        assert!(!serialized.contains("type-args"));
        assert!(!serialized.contains("resources"));
        assert!(!serialized.contains("callables"));
        let compiler_selection = manifest
            .compiler_component_selection()
            .unwrap()
            .expect("compiler selection should exist");
        let component = manifest.component.expect("component config should load");

        assert_eq!(component.schema, COMPONENT_SCHEMA_VERSION);
        assert_eq!(component.package, "example:calculator@0.1.0");
        assert_eq!(component.world, "calculator");
        assert_eq!(component.abi, COMPONENT_ABI_PROFILE);
        assert_eq!(
            component.exports,
            vec![ComponentExport {
                interface: "api".to_string(),
                source: "add".to_string(),
                name: "add".to_string(),
                type_args: BTreeMap::new(),
            }]
        );
        assert_eq!(
            compiler_selection,
            restrict_lang::component::ComponentSelection::v1(
                "example:calculator@0.1.0",
                "calculator",
                COMPONENT_ABI_PROFILE,
                vec![restrict_lang::component::ComponentExportSelection::new(
                    "api", "add", "add",
                )],
            )
        );
    }

    #[test]
    fn component_selection_rejects_unknown_schema_profile_and_fields() {
        assert_invalid_component(
            &VALID_COMPONENT.replace("schema = 1", "schema = 3"),
            "Unsupported component schema 3",
        );
        assert_invalid_component(
            &VALID_COMPONENT.replace("canonical-v1-sync", "canonical-v2-async"),
            "Unsupported component ABI profile 'canonical-v2-async'",
        );
        assert_invalid_component(
            &VALID_COMPONENT.replace(
                "world = \"calculator\"",
                "world = \"calculator\"\nmode = \"preview\"",
            ),
            "unknown field `mode`",
        );
        assert_invalid_component(
            &VALID_COMPONENT.replace("name = \"add\"", "name = \"add\"\ntype_args = [\"Int32\"]"),
            "unknown field `type_args`",
        );
        assert_invalid_component(
            &VALID_COMPONENT.replace(
                "name = \"add\"",
                "name = \"add\"\ntype-args = { T = \"String\" }",
            ),
            "component schema 1 does not accept component.exports.type-args",
        );
    }

    #[test]
    fn component_selection_requires_nonempty_unique_exports() {
        let no_exports = r#"
[component]
schema = 1
package = "example:calculator@0.1.0"
world = "calculator"
abi = "canonical-v1-sync"
"#;
        assert_invalid_component(no_exports, "component.exports must contain at least one");

        let duplicate_source = format!(
            "{VALID_COMPONENT}\n[[component.exports]]\ninterface = \"admin\"\nsource = \"add\"\nname = \"add-admin\"\n"
        );
        assert_invalid_component(&duplicate_source, "Duplicate component export source 'add'");

        let duplicate_wit_name = format!(
            "{VALID_COMPONENT}\n[[component.exports]]\ninterface = \"api\"\nsource = \"subtract\"\nname = \"add\"\n"
        );
        assert_invalid_component(
            &duplicate_wit_name,
            "Duplicate component WIT export 'api.add'",
        );
    }

    #[test]
    fn component_selection_validates_package_and_identifiers() {
        for invalid_package in [
            "calculator@0.1.0",
            "example:calculator",
            "example:calculator@next",
            "Example:calculator@0.1.0",
            "example:calculator:extra@0.1.0",
        ] {
            assert_invalid_component(
                &VALID_COMPONENT.replace("example:calculator@0.1.0", invalid_package),
                "Invalid component.package",
            );
        }

        for (old, new, expected) in [
            (
                "world = \"calculator\"",
                "world = \"Calculator\"",
                "Invalid component.world",
            ),
            (
                "interface = \"api\"",
                "interface = \"public_api\"",
                "Invalid component.exports[0].interface",
            ),
            (
                "name = \"add\"",
                "name = \"add_value\"",
                "Invalid component.exports[0].name",
            ),
            (
                "source = \"add\"",
                "source = \"release.add\"",
                "expected one non-keyword root-module Restrict identifier",
            ),
            (
                "world = \"calculator\"",
                "world = \"string\"",
                "Invalid component.world",
            ),
            (
                "interface = \"api\"",
                "interface = \"list\"",
                "Invalid component.exports[0].interface",
            ),
            (
                "name = \"add\"",
                "name = \"async\"",
                "Invalid component.exports[0].name",
            ),
        ] {
            assert_invalid_component(&VALID_COMPONENT.replace(old, new), expected);
        }
    }

    #[test]
    fn component_package_identity_matches_the_source_package() {
        assert_invalid_component(
            &VALID_COMPONENT.replace("example:calculator@0.1.0", "example:other-package@0.1.0"),
            "component.package name 'other-package' must match package.name 'calculator'",
        );
        assert_invalid_component(
            &VALID_COMPONENT.replace("example:calculator@0.1.0", "example:calculator@0.2.0"),
            "component.package version '0.2.0' must match package.version '0.1.0'",
        );
    }

    #[test]
    fn schema_v2_normalizes_generic_resource_and_callable_contracts() {
        use restrict_lang::component_selection_v2 as contract;

        let manifest = load_manifest_text(VALID_COMPONENT_V2).unwrap();
        let compiler_contract = manifest
            .compiler_component_contract()
            .unwrap()
            .expect("component contract should exist");

        assert_eq!(
            compiler_contract.schema_version,
            COMPONENT_SCHEMA_VERSION_V2
        );
        assert_eq!(compiler_contract.exports.len(), 1);
        assert_eq!(
            compiler_contract.exports[0].type_arguments,
            vec![contract::ComponentTypeArgumentContract {
                parameter: "T".to_string(),
                ty: contract::ClosedComponentType::String,
            }]
        );
        assert_eq!(compiler_contract.resources.len(), 1);
        assert_eq!(
            compiler_contract.resources[0].destructor.visibility,
            contract::ComponentSourceVisibility::Private
        );
        assert_eq!(
            compiler_contract.resources[0]
                .methods
                .iter()
                .map(|method| method.receiver)
                .collect::<Vec<_>>(),
            vec![
                contract::ComponentResourceReceiver::Own,
                contract::ComponentResourceReceiver::Borrow,
            ]
        );
        assert_eq!(compiler_contract.callables.len(), 1);
        assert_eq!(
            compiler_contract.callables[0].multiplicity,
            contract::ComponentCallableMultiplicity::Once
        );

        let public_destructor = load_manifest_text(&VALID_COMPONENT_V2.replace(
            "destructor-visibility = \"private\"",
            "destructor-visibility = \"public\"",
        ))
        .unwrap()
        .compiler_component_contract()
        .unwrap()
        .unwrap();
        assert_eq!(
            public_destructor.resources[0].destructor.visibility,
            contract::ComponentSourceVisibility::Public
        );

        let error = manifest.compiler_component_selection().unwrap_err();
        assert!(error
            .to_string()
            .contains("cannot be represented by the legacy schema-v1 selection"));
    }

    #[test]
    fn schema_v2_allows_distinct_explicit_instances_of_one_generic() {
        let source = format!(
            "{VALID_COMPONENT_V2}\n[[component.exports]]\ninterface = \"api\"\nsource = \"identity\"\nname = \"identity-int\"\ntype-args = {{ T = \"Int32\" }}\n"
        );
        let contract = load_manifest_text(&source)
            .unwrap()
            .compiler_component_contract()
            .unwrap()
            .unwrap();

        assert_eq!(contract.exports.len(), 2);
        assert_ne!(
            contract.exports[0].type_arguments,
            contract.exports[1].type_arguments
        );
    }

    #[test]
    fn schema_v2_reports_closed_type_and_multiplicity_errors() {
        assert_invalid_component(
            &VALID_COMPONENT_V2.replace("T = \"String\"", "T = \"File<~t>\""),
            "temporal types are not closed synchronous type expressions",
        );
        assert_invalid_component(
            &VALID_COMPONENT_V2.replace("multiplicity = \"once\"", "multiplicity = \"many\""),
            "initially accepts only 'once'",
        );
    }

    #[test]
    fn schema_v2_rejects_duplicate_external_and_resource_method_identities() {
        assert_invalid_component(
            &VALID_COMPONENT_V2.replace("name = \"transform\"", "name = \"file-handle\""),
            "duplicate component external declaration 'api.file-handle'",
        );
        assert_invalid_component(
            &VALID_COMPONENT_V2.replace(
                "{ source = \"finish_file\", name = \"finish\", receiver = \"own\" }",
                "{ source = \"read_file\", name = \"finish\", receiver = \"own\" }",
            ),
            "duplicate component.resources[0] method source 'read_file'",
        );
        assert_invalid_component(
            &VALID_COMPONENT_V2.replace("source = \"finish_file\"", "source = \"close_file\""),
            "destructor source 'close_file' cannot also be declared as a resource method",
        );
    }

    #[cfg(unix)]
    #[test]
    fn manifest_save_does_not_follow_a_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let outside = temp.path().join("outside.toml");
        let path = temp.path().join(WARDER_MANIFEST_FILENAME);
        std::fs::write(&outside, "keep\n").unwrap();
        symlink(&outside, &path).unwrap();

        let error = Manifest::new("app").save(&path).unwrap_err();

        assert!(error.to_string().contains("non-regular Warder manifest"));
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
