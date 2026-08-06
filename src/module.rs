use crate::ast::{
    AssignStmt, BindDecl, BlockExpr, CallExpr, CloneExpr, Expr, ExprKind, FieldInit, FormDecl,
    FunDecl, ImplBlock, ImportItems, MatchArm, MatchExpr, Pattern, PipeExpr, PipeTarget, Program,
    PrototypeCloneExpr, RecordDecl, RecordLit, Stmt, TakesDecl, ThenExpr, TopDecl, Type, WhileExpr,
    WithExpr, WithLifetimeExpr,
};
use crate::diagnostics::format_parse_error;
use crate::parser::parse_program;
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const UNSUPPORTED_STD_SOURCE_IMPORT_ERROR: &str =
    "standard-library source imports are unsupported in v0.0.1; std helpers are compiler-registered and available without importing std aggregators";

type ModuleSymbol = (Vec<String>, String);
const INTERNAL_MODULE_NAME_PREFIX: &str = "__rl$mod";

#[derive(Debug, Clone)]
pub struct Module {
    pub path: PathBuf,
    pub name: Vec<String>,
    pub program: Program,
    pub exports: HashMap<String, TopDecl>,
}

pub struct ModuleResolver {
    modules: HashMap<Vec<String>, Module>,
    search_paths: Vec<PathBuf>,
    package_roots: HashMap<String, PathBuf>,
    module_sources: HashMap<Vec<String>, String>,
    module_source_conflicts: HashSet<Vec<String>>,
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            // Explicit roots take precedence over the process working
            // directory. The latter remains a fallback for direct CLI use.
            search_paths: Vec::new(),
            package_roots: HashMap::new(),
            module_sources: HashMap::new(),
            module_source_conflicts: HashSet::new(),
        }
    }

    pub fn add_search_path(&mut self, path: PathBuf) -> Result<()> {
        if !path.is_dir() {
            bail!("Module search root is not a directory: {}", path.display());
        }

        let canonical_path = fs::canonicalize(&path).with_context(|| {
            format!(
                "Failed to canonicalize module search root: {}",
                path.display()
            )
        })?;
        if self.search_paths.contains(&canonical_path) {
            return Ok(());
        }
        if let Some((namespace, package_root)) = self
            .package_roots
            .iter()
            .find(|(_, package_root)| roots_overlap(&canonical_path, package_root))
        {
            bail!(
                "Module search root {} overlaps package source root {} registered as namespace '{}'",
                canonical_path.display(),
                package_root.display(),
                namespace
            );
        }

        self.search_paths.push(canonical_path);
        Ok(())
    }

    /// Bind one source-level namespace to a package's `src/` directory.
    ///
    /// The namespace root resolves to `lib.rl`; subsequent path segments map
    /// to sibling files and directories below the registered source root.
    pub fn add_package_root(&mut self, namespace: String, source_dir: PathBuf) -> Result<()> {
        if !crate::lexer::is_source_identifier(&namespace) {
            bail!(
                "Invalid package namespace '{}': expected a non-keyword Restrict identifier",
                namespace
            );
        }
        if namespace == "std" {
            bail!("Package namespace 'std' is reserved for the standard library");
        }
        if self.package_roots.contains_key(&namespace) {
            bail!(
                "Duplicate package namespace '{}': already registered",
                namespace
            );
        }
        if self
            .modules
            .keys()
            .chain(self.module_sources.keys())
            .chain(self.module_source_conflicts.iter())
            .any(|module_path| module_path.first() == Some(&namespace))
        {
            bail!(
                "Package namespace '{}' is already used by a resolved or virtual source module",
                namespace
            );
        }
        if !source_dir.is_dir() {
            bail!(
                "Package source root for '{}' is not a directory: {}",
                namespace,
                source_dir.display()
            );
        }

        let canonical_source_dir = fs::canonicalize(&source_dir).with_context(|| {
            format!(
                "Failed to canonicalize package source root for '{}': {}",
                namespace,
                source_dir.display()
            )
        })?;
        if let Some(search_root) = self
            .search_paths
            .iter()
            .find(|search_root| roots_overlap(&canonical_source_dir, search_root))
        {
            bail!(
                "Package source root {} for namespace '{}' overlaps configured module search root {}",
                canonical_source_dir.display(),
                namespace,
                search_root.display()
            );
        }
        if let Some((existing_namespace, _)) = self
            .package_roots
            .iter()
            .find(|(_, existing_root)| roots_overlap(&canonical_source_dir, existing_root))
        {
            bail!(
                "Package source root {} overlaps the source root registered as namespace '{}'",
                canonical_source_dir.display(),
                existing_namespace
            );
        }

        self.package_roots.insert(namespace, canonical_source_dir);
        Ok(())
    }

    pub fn add_module_source(&mut self, module_path: Vec<String>, source: String) {
        if module_path
            .first()
            .is_some_and(|namespace| self.package_roots.contains_key(namespace))
        {
            self.module_source_conflicts.insert(module_path);
            return;
        }
        let already_resolved = self.modules.contains_key(&module_path);
        let replaced = self
            .module_sources
            .insert(module_path.clone(), source)
            .is_some();
        if already_resolved || replaced {
            // Preserve the original infallible API while ensuring a duplicate
            // cannot be accepted by a later resolver call.
            self.module_source_conflicts.insert(module_path);
        }
    }

    pub fn try_add_module_source(
        &mut self,
        module_path: Vec<String>,
        source: String,
    ) -> Result<()> {
        if module_path
            .first()
            .is_some_and(|namespace| self.package_roots.contains_key(namespace))
        {
            bail!(
                "Virtual module {} conflicts with its configured package namespace",
                module_path.join(".")
            );
        }
        if self.module_sources.contains_key(&module_path)
            || self.modules.contains_key(&module_path)
            || self.module_source_conflicts.contains(&module_path)
        {
            bail!(
                "Duplicate or already resolved module source for canonical module {}",
                module_path.join(".")
            );
        }

        self.module_sources.insert(module_path, source);
        Ok(())
    }

    pub fn add_module_source_key(&mut self, module_key: &str, source: String) -> Result<()> {
        let module_path = parse_module_source_key(module_key)?;
        self.try_add_module_source(module_path, source)
    }

    pub fn resolve_module(&mut self, module_path: &[String]) -> Result<Vec<String>> {
        let mut visiting = Vec::new();
        let mut staged_modules = HashMap::new();
        self.resolve_module_transaction(module_path, &mut visiting, &mut staged_modules)?;
        self.modules.extend(staged_modules);
        Ok(module_path.to_vec())
    }

    fn resolve_module_transaction(
        &self,
        module_path: &[String],
        visiting: &mut Vec<Vec<String>>,
        staged_modules: &mut HashMap<Vec<String>, Module>,
    ) -> Result<()> {
        if is_reserved_std_module_path(module_path) {
            bail!("{UNSUPPORTED_STD_SOURCE_IMPORT_ERROR}");
        }

        if self.module_source_conflicts.contains(module_path) {
            bail!(
                "Duplicate module source for canonical module {}",
                module_path.join(".")
            );
        }

        if self.modules.contains_key(module_path) || staged_modules.contains_key(module_path) {
            return Ok(());
        }

        if let Some(cycle_start) = visiting.iter().position(|path| path == module_path) {
            let mut cycle = visiting[cycle_start..]
                .iter()
                .map(|path| path.join("."))
                .collect::<Vec<_>>();
            cycle.push(module_path.join("."));
            bail!("Cyclic module import detected: {}", cycle.join(" -> "));
        }

        visiting.push(module_path.to_vec());

        let (source_path, content) = self.load_module_source(module_path)?;
        if let Some(existing_module) = self
            .modules
            .values()
            .chain(staged_modules.values())
            .find(|module| module.path == source_path && module.name != module_path)
        {
            bail!(
                "Module file {} would have multiple canonical identities: {} and {}",
                source_path.display(),
                existing_module.name.join("."),
                module_path.join(".")
            );
        }

        let (remaining, mut program) = parse_program(&content).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse module {} at {}: {}",
                module_path.join("."),
                source_path.display(),
                format_parse_error(&content, e)
            )
        })?;

        if !remaining.trim().is_empty() {
            bail!("Unexpected content after module: {}", remaining);
        }

        self.qualify_package_local_imports(module_path, &mut program);

        // Collect imports to process later
        let imports = program.imports.clone();

        // Build export table
        let mut exports = HashMap::new();
        let mut regular_decls = Vec::new();
        let mut declared_names = HashSet::new();

        for decl in program.declarations {
            reject_reserved_prelude_form_decl(&decl, module_path)?;
            match decl {
                TopDecl::Export(export_decl) => {
                    let name = get_decl_name(&export_decl.item)?;
                    if exports.contains_key(&name) {
                        bail!(
                            "Duplicate export '{}' in module {}",
                            name,
                            module_path.join(".")
                        );
                    }
                    if !declared_names.insert(name.clone()) {
                        bail!(
                            "Duplicate declaration '{}' in module {}",
                            name,
                            module_path.join(".")
                        );
                    }
                    exports.insert(name, *export_decl.item);
                }
                decl => {
                    // Impl and takes blocks attach behavior to an existing
                    // receiver and do not declare that receiver again.
                    let declared_name = match &decl {
                        TopDecl::Impl(_) | TopDecl::Takes(_) => None,
                        _ => get_top_decl_name_for_collision(&decl)?,
                    };
                    if let Some(name) = declared_name {
                        if !declared_names.insert(name.clone()) {
                            bail!(
                                "Duplicate declaration '{}' in module {}",
                                name,
                                module_path.join(".")
                            );
                        }
                    }
                    regular_decls.push(decl);
                }
            }
        }

        program.declarations = regular_decls;

        let module = Module {
            path: source_path,
            name: module_path.to_vec(),
            program,
            exports,
        };

        // Resolve the complete dependency closure before committing anything
        // to the resolver cache. A failed parse/load therefore remains
        // retryable on the same resolver instance.
        for import in &imports {
            self.resolve_module_transaction(&import.module_path, visiting, staged_modules)?;
        }

        visiting.pop();
        staged_modules.insert(module_path.to_vec(), module);
        Ok(())
    }

    fn load_module_source(&self, module_path: &[String]) -> Result<(PathBuf, String)> {
        if let Some(source) = self.module_sources.get(module_path) {
            return Ok((virtual_module_path(module_path), source.clone()));
        }

        let file_path = self.find_module_file(module_path)?;
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read module file: {:?}", file_path))?;

        Ok((file_path, content))
    }

    pub fn get_module(&self, module_path: &[String]) -> Option<&Module> {
        self.modules.get(module_path)
    }

    pub fn resolve_program_imports(&mut self, mut program: Program) -> Result<Program> {
        if program.imports.is_empty() {
            return Ok(program);
        }

        let mut declarations = Vec::new();
        let mut emitted_names = HashSet::new();
        let mut imported_names = HashMap::new();
        let mut declared_names = HashMap::new();
        let mut root_aliases = HashMap::<ModuleSymbol, String>::new();

        for decl in &program.declarations {
            if let Some(name) = get_top_decl_name_for_collision(decl)? {
                declared_names.insert(name, "root module".to_string());
            }
        }

        for import in &program.imports {
            self.resolve_module(&import.module_path)?;

            let requested_names =
                self.get_requested_import_names(&import.module_path, &import.items)?;
            for name in &requested_names {
                if let Some(previous) = imported_names.get(name) {
                    bail!(
                        "Import name collision for '{}': already imported from {}",
                        name,
                        previous
                    );
                }

                if declared_names.contains_key(name) {
                    bail!(
                        "Import name collision for '{}': root module already declares this name",
                        name
                    );
                }

                imported_names.insert(name.clone(), import.module_path.join("."));
                root_aliases.insert((import.module_path.clone(), name.clone()), name.clone());
            }
        }

        // The root-visible alias plan is complete before traversing any
        // dependency. Every reference to one source declaration therefore
        // receives the same name, regardless of split or transitive imports.
        let mut emitted_modules = HashSet::new();
        for import in &program.imports {
            for decl in self.get_import_closure_decls(
                &import.module_path,
                &root_aliases,
                &mut emitted_modules,
            )? {
                if let Some(key) = get_top_decl_emit_key(&decl)? {
                    if !emitted_names.insert(key.clone()) {
                        bail!("Resolved module declaration collision for {key}");
                    }
                }

                declarations.push(decl);
            }
        }

        declarations.extend(program.declarations);
        program.imports.clear();
        program.declarations = declarations;

        // Imported declarations were numbered per source file (and renaming
        // rebuilds nodes with dummy ids), so renumber the spliced program to
        // restore one dense, program-wide NodeId space.
        crate::ast::assign_node_ids(&mut program);

        Ok(program)
    }

    fn find_module_file(&self, module_path: &[String]) -> Result<PathBuf> {
        if let Some((namespace, source_root)) = module_path
            .first()
            .and_then(|namespace| self.package_roots.get_key_value(namespace))
        {
            if module_path.len() == 2 && module_path[1] == "lib" {
                bail!(
                    "Package module {} aliases the namespace root; import '{}' instead",
                    module_path.join("."),
                    namespace
                );
            }
            let relative_path = if module_path.len() == 1 {
                PathBuf::from("lib.rl")
            } else {
                let mut path = module_path[1..].iter().collect::<PathBuf>();
                path.set_extension("rl");
                path
            };
            let candidate = source_root.join(relative_path);
            if candidate.is_file() {
                let canonical_candidate = fs::canonicalize(&candidate).with_context(|| {
                    format!(
                        "Failed to canonicalize package module file: {}",
                        candidate.display()
                    )
                })?;
                if !canonical_candidate.starts_with(source_root) {
                    bail!(
                        "Package module {} escapes source root {} through {}",
                        module_path.join("."),
                        source_root.display(),
                        candidate.display()
                    );
                }
                return Ok(canonical_candidate);
            }

            bail!(
                "Package module {} was not found in namespace '{}'; expected {}",
                module_path.join("."),
                namespace,
                candidate.display()
            );
        }

        let relative_path = module_path.join("/") + ".rl";
        let mut explicit_candidates = Vec::new();

        for search_path in &self.search_paths {
            let full_path = search_path.join(&relative_path);
            if full_path.is_file() {
                let canonical = fs::canonicalize(&full_path).with_context(|| {
                    format!(
                        "Failed to canonicalize module file: {}",
                        full_path.display()
                    )
                })?;
                self.reject_package_owned_file(module_path, &canonical)?;
                if !explicit_candidates.contains(&canonical) {
                    explicit_candidates.push(canonical);
                }
            }
        }

        match explicit_candidates.as_slice() {
            [candidate] => return Ok(candidate.clone()),
            [] => {}
            candidates => {
                let mut paths = candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>();
                paths.sort();
                bail!(
                    "Ambiguous module {} found in configured search roots: {}",
                    module_path.join("."),
                    paths.join(", ")
                );
            }
        }

        let cwd_candidate = PathBuf::from(".").join(&relative_path);
        if cwd_candidate.is_file() {
            let canonical = fs::canonicalize(&cwd_candidate).with_context(|| {
                format!(
                    "Failed to canonicalize module file: {}",
                    cwd_candidate.display()
                )
            })?;
            self.reject_package_owned_file(module_path, &canonical)?;
            return Ok(canonical);
        }

        bail!("Module not found: {}", module_path.join("."))
    }

    fn reject_package_owned_file(&self, module_path: &[String], file: &Path) -> Result<()> {
        if let Some((namespace, _)) = self
            .package_roots
            .iter()
            .find(|(_, source_root)| file.starts_with(source_root))
        {
            bail!(
                "Module {} resolves to {} inside package namespace '{}'; import it through the '{}' namespace",
                module_path.join("."),
                file.display(),
                namespace,
                namespace
            );
        }
        Ok(())
    }

    fn qualify_package_local_imports(&self, module_path: &[String], program: &mut Program) {
        let Some(namespace) = module_path
            .first()
            .filter(|namespace| self.package_roots.contains_key(*namespace))
        else {
            return;
        };

        for import in &mut program.imports {
            let is_explicit_self_import = import
                .module_path
                .first()
                .is_some_and(|root| root == namespace);
            let is_reserved_std_import = is_reserved_std_module_path(&import.module_path);
            if !is_explicit_self_import && !is_reserved_std_import {
                import.module_path.insert(0, namespace.clone());
            }
        }
    }

    pub fn get_imported_items(
        &self,
        module_path: &[String],
        items: &ImportItems,
    ) -> Result<Vec<(String, TopDecl)>> {
        let module = self
            .modules
            .get(module_path)
            .with_context(|| format!("Module not resolved: {}", module_path.join(".")))?;

        match items {
            ImportItems::All => {
                let mut exports = module
                    .exports
                    .iter()
                    .map(|(name, decl)| (name.clone(), decl.clone()))
                    .collect::<Vec<_>>();
                exports.sort_by(|left, right| left.0.cmp(&right.0));
                Ok(exports)
            }
            ImportItems::Named(names) => {
                let mut result = Vec::new();
                for name in names {
                    let decl = module.exports.get(name).with_context(|| {
                        format!(
                            "Export '{}' not found in module {}",
                            name,
                            module_path.join(".")
                        )
                    })?;
                    result.push((name.clone(), decl.clone()));
                }
                Ok(result)
            }
        }
    }

    fn get_requested_import_names(
        &self,
        module_path: &[String],
        items: &ImportItems,
    ) -> Result<Vec<String>> {
        let module = self
            .modules
            .get(module_path)
            .with_context(|| format!("Module not resolved: {}", module_path.join(".")))?;

        match items {
            ImportItems::All => {
                let mut names = module.exports.keys().cloned().collect::<Vec<_>>();
                names.sort();
                Ok(names)
            }
            ImportItems::Named(names) => {
                for name in names {
                    if !module.exports.contains_key(name) {
                        bail!(
                            "Export '{}' not found in module {}",
                            name,
                            module_path.join(".")
                        );
                    }
                }
                Ok(names.clone())
            }
        }
    }

    fn get_import_closure_decls(
        &self,
        module_path: &[String],
        root_aliases: &HashMap<ModuleSymbol, String>,
        emitted_modules: &mut HashSet<Vec<String>>,
    ) -> Result<Vec<TopDecl>> {
        let mut visiting = Vec::new();

        self.get_import_closure_decls_with_aliases(
            module_path,
            root_aliases,
            &mut visiting,
            emitted_modules,
        )
    }

    fn get_import_closure_decls_with_aliases(
        &self,
        module_path: &[String],
        root_aliases: &HashMap<ModuleSymbol, String>,
        visiting: &mut Vec<Vec<String>>,
        emitted_modules: &mut HashSet<Vec<String>>,
    ) -> Result<Vec<TopDecl>> {
        let module_key = module_path.to_vec();
        if emitted_modules.contains(&module_key) {
            return Ok(Vec::new());
        }

        if let Some(cycle_start) = visiting.iter().position(|path| path == module_path) {
            let mut cycle = visiting[cycle_start..]
                .iter()
                .map(|path| path.join("."))
                .collect::<Vec<_>>();
            cycle.push(module_path.join("."));
            bail!("Cyclic module import detected: {}", cycle.join(" -> "));
        }
        visiting.push(module_key.clone());

        let module = self
            .modules
            .get(module_path)
            .cloned()
            .with_context(|| format!("Module not resolved: {}", module_path.join(".")))?;
        let mut rename_map = HashMap::new();
        let mut declarations = Vec::new();

        for import in &module.program.imports {
            let imported_names =
                self.get_requested_import_names(&import.module_path, &import.items)?;
            let dependency_aliases = imported_names
                .iter()
                .map(|name| {
                    (
                        name.clone(),
                        module_symbol_alias(root_aliases, &import.module_path, name),
                    )
                })
                .collect::<HashMap<_, _>>();

            declarations.extend(self.get_import_closure_decls_with_aliases(
                &import.module_path,
                root_aliases,
                visiting,
                emitted_modules,
            )?);

            for (name, alias) in dependency_aliases {
                insert_rename(&mut rename_map, name, alias, module_path)?;
            }
        }

        for decl in &module.program.declarations {
            if let Some(name) = get_top_decl_name_for_collision(decl)? {
                let alias = module_symbol_alias(root_aliases, module_path, &name);
                insert_rename(&mut rename_map, name.clone(), alias, module_path)?;
            }
        }

        for name in module.exports.keys() {
            let alias = module_symbol_alias(root_aliases, module_path, name);
            insert_rename(&mut rename_map, name.clone(), alias, module_path)?;
        }

        for decl in &module.program.declarations {
            declarations.push(rename_top_decl(decl.clone(), &rename_map)?);
        }

        let mut exports = module.exports.iter().collect::<Vec<_>>();
        exports.sort_by(|left, right| left.0.cmp(right.0));
        for (_, decl) in exports {
            let renamed = rename_top_decl(decl.clone(), &rename_map)?;
            if matches!(
                decl,
                TopDecl::Record(_) | TopDecl::Enum(_) | TopDecl::Form(_)
            ) {
                // Imported public source types stay non-re-exported at the module
                // graph level, but the flattened compiler program must retain
                // their source-public provenance for downstream validation.
                // A source-level type export has no direct Wasm export.
                declarations.push(TopDecl::Export(crate::ast::ExportDecl {
                    item: Box::new(renamed),
                }));
            } else {
                declarations.push(renamed);
            }
        }

        visiting.pop();
        emitted_modules.insert(module_key);
        Ok(declarations)
    }
}

fn reject_reserved_prelude_form_decl(decl: &TopDecl, module_path: &[String]) -> Result<()> {
    let decl = match decl {
        TopDecl::Export(export) => export.item.as_ref(),
        other => other,
    };
    if let TopDecl::Form(form) = decl {
        if matches!(form.name.as_str(), "Display" | "Container") {
            bail!(
                "Form '{}' is compiler-provided or compiler-internal and cannot be redeclared in module {}",
                form.name,
                module_path.join(".")
            );
        }
    }
    Ok(())
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

pub fn resolve_program_imports_for_file(program: Program, source_file: &Path) -> Result<Program> {
    resolve_program_imports_for_file_with_package_roots(program, source_file, &[])
}

pub fn resolve_program_imports_for_file_with_package_roots(
    program: Program,
    source_file: &Path,
    package_roots: &[(String, PathBuf)],
) -> Result<Program> {
    let base_dir = source_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());

    let mut resolver = ModuleResolver::new();
    if let Some(base_dir) = base_dir {
        resolver.add_search_path(base_dir.to_path_buf())?;
    }
    for (namespace, source_dir) in package_roots {
        resolver.add_package_root(namespace.clone(), source_dir.clone())?;
    }

    if program.imports.is_empty() {
        return Ok(program);
    }

    resolver.resolve_program_imports(program)
}

pub fn resolve_program_imports_with_base_dir(
    program: Program,
    base_dir: Option<&Path>,
) -> Result<Program> {
    if program.imports.is_empty() {
        return Ok(program);
    }

    let mut resolver = ModuleResolver::new();
    if let Some(base_dir) = base_dir {
        resolver.add_search_path(base_dir.to_path_buf())?;
    }

    resolver.resolve_program_imports(program)
}

pub fn resolve_program_imports_with_module_source_map(
    program: Program,
    module_sources: HashMap<String, String>,
) -> Result<Program> {
    if program.imports.is_empty() {
        return Ok(program);
    }

    let mut resolver = ModuleResolver::new();
    for (module_key, source) in module_sources {
        resolver.add_module_source_key(&module_key, source)?;
    }

    resolver.resolve_program_imports(program)
}

pub fn parse_module_source_key(module_key: &str) -> Result<Vec<String>> {
    let trimmed = module_key.trim();
    let module_name = trimmed.strip_suffix(".rl").unwrap_or(trimmed);

    let path = module_name
        .split(['.', '/'])
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    if path.is_empty() {
        bail!("Module source key must name a module");
    }

    Ok(path)
}

fn virtual_module_path(module_path: &[String]) -> PathBuf {
    PathBuf::from(format!("<module:{}>", module_path.join(".")))
}

fn is_reserved_std_module_path(module_path: &[String]) -> bool {
    module_path.first().is_some_and(|part| part == "std")
}

fn insert_rename(
    rename_map: &mut HashMap<String, String>,
    source_name: String,
    renamed_name: String,
    module_path: &[String],
) -> Result<()> {
    if let Some(existing) = rename_map.insert(source_name.clone(), renamed_name.clone()) {
        if existing != renamed_name {
            bail!(
                "Module name collision for '{}' in {}",
                source_name,
                module_path.join(".")
            );
        }
    }

    Ok(())
}

fn module_symbol_alias(
    root_aliases: &HashMap<ModuleSymbol, String>,
    module_path: &[String],
    name: &str,
) -> String {
    root_aliases
        .get(&(module_path.to_vec(), name.to_string()))
        .cloned()
        .unwrap_or_else(|| mangle_module_name(module_path, name))
}

fn mangle_module_name(module_path: &[String], name: &str) -> String {
    // `$` is valid in a WAT identifier but cannot be written by the Restrict
    // lexer, so source declarations cannot collide with this namespace.
    let mut mangled = String::from(INTERNAL_MODULE_NAME_PREFIX);
    for part in module_path.iter().map(String::as_str).chain([name]) {
        mangled.push('_');
        mangled.push_str(&part.len().to_string());
        mangled.push('_');
        mangled.push_str(part);
    }
    mangled
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_internal_module_name(name: &str) -> bool {
    name.starts_with(INTERNAL_MODULE_NAME_PREFIX)
}

fn rename_name(name: String, rename_map: &HashMap<String, String>) -> String {
    rename_map.get(&name).cloned().unwrap_or(name)
}

fn rename_type(
    ty: Type,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
) -> Type {
    match ty {
        Type::Named(name) if type_params.contains(&name) => Type::Named(name),
        Type::Named(name) => Type::Named(rename_name(name, rename_map)),
        Type::Generic(name, params) => Type::Generic(
            rename_name(name, rename_map),
            params
                .into_iter()
                .map(|param| rename_type(param, rename_map, type_params))
                .collect(),
        ),
        Type::Function(params, return_type) => Type::Function(
            params
                .into_iter()
                .map(|param| rename_type(param, rename_map, type_params))
                .collect(),
            Box::new(rename_type(*return_type, rename_map, type_params)),
        ),
        Type::Temporal(name, temporals) => Type::Temporal(rename_name(name, rename_map), temporals),
    }
}

fn rename_top_decl(decl: TopDecl, rename_map: &HashMap<String, String>) -> Result<TopDecl> {
    match decl {
        TopDecl::Record(record) => Ok(TopDecl::Record(rename_record_decl(record, rename_map))),
        TopDecl::Enum(enum_decl) => Ok(TopDecl::Enum(rename_enum_decl(enum_decl, rename_map))),
        TopDecl::Form(form) => Ok(TopDecl::Form(rename_form_decl(form, rename_map))),
        TopDecl::Takes(takes) => Ok(TopDecl::Takes(rename_takes_decl(takes, rename_map))),
        TopDecl::Function(function) => Ok(TopDecl::Function(rename_fun_decl(function, rename_map))),
        TopDecl::Binding(binding) => {
            Ok(TopDecl::Binding(rename_bind_decl_top(binding, rename_map)))
        }
        TopDecl::Impl(impl_block) => Ok(TopDecl::Impl(rename_impl_block(impl_block, rename_map))),
        TopDecl::Context(context) => Ok(TopDecl::Context(rename_context_decl(context, rename_map))),
        TopDecl::Export(export_decl) => Ok(TopDecl::Export(crate::ast::ExportDecl {
            item: Box::new(rename_top_decl(*export_decl.item, rename_map)?),
        })),
    }
}

fn rename_enum_decl(
    mut enum_decl: crate::ast::EnumDecl,
    rename_map: &HashMap<String, String>,
) -> crate::ast::EnumDecl {
    let type_params = HashSet::new();
    enum_decl.name = rename_name(enum_decl.name, rename_map);
    for variant in &mut enum_decl.variants {
        variant.payload = variant
            .payload
            .take()
            .map(|payload| rename_type(payload, rename_map, &type_params));
    }
    enum_decl
}

fn rename_record_decl(mut record: RecordDecl, rename_map: &HashMap<String, String>) -> RecordDecl {
    let type_params = record
        .type_params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    record.name = rename_name(record.name, rename_map);
    for type_param in &mut record.type_params {
        for form_name in &mut type_param.of_forms {
            *form_name = rename_name(form_name.clone(), rename_map);
        }
    }
    for field in &mut record.fields {
        field.ty = rename_type(field.ty.clone(), rename_map, &type_params);
    }
    record.parent_hash = record
        .parent_hash
        .map(|parent| rename_name(parent, rename_map));
    record
}

fn rename_form_decl(mut form: FormDecl, rename_map: &HashMap<String, String>) -> FormDecl {
    // `Self` is a form-local placeholder, not a module declaration.
    let type_params = HashSet::from(["Self".to_string()]);
    form.name = rename_name(form.name, rename_map);
    for method in &mut form.methods {
        for param in &mut method.params {
            param.ty = rename_type(param.ty.clone(), rename_map, &type_params);
        }
        method.return_type = rename_type(method.return_type.clone(), rename_map, &type_params);
    }
    form
}

fn rename_takes_decl(mut takes: TakesDecl, rename_map: &HashMap<String, String>) -> TakesDecl {
    takes.target = rename_name(takes.target, rename_map);
    takes.form_name = rename_name(takes.form_name, rename_map);
    takes.functions = takes
        .functions
        .into_iter()
        .map(|function| {
            // A method selector belongs to the form contract and must remain
            // stable even when declarations referenced by its signature/body
            // are module-qualified.
            let selector = function.name.clone();
            let mut function = rename_fun_decl(function, rename_map);
            function.name = selector;
            function
        })
        .collect();
    takes
}

fn rename_fun_decl(mut function: FunDecl, rename_map: &HashMap<String, String>) -> FunDecl {
    let type_params = function
        .type_params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    function.name = rename_name(function.name, rename_map);
    for type_param in &mut function.type_params {
        for form_name in &mut type_param.of_forms {
            *form_name = rename_name(form_name.clone(), rename_map);
        }
    }
    for param in &mut function.params {
        param.ty = rename_type(param.ty.clone(), rename_map, &type_params);
    }
    function.return_type = function
        .return_type
        .map(|ty| rename_type(ty, rename_map, &type_params));

    let mut bound = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    function.body = rename_block_expr(function.body, rename_map, &type_params, &mut bound);
    function
}

fn rename_impl_block(mut impl_block: ImplBlock, rename_map: &HashMap<String, String>) -> ImplBlock {
    impl_block.target = rename_name(impl_block.target, rename_map);
    impl_block.functions = impl_block
        .functions
        .into_iter()
        .map(|function| rename_fun_decl(function, rename_map))
        .collect();
    impl_block
}

fn rename_context_decl(
    mut context: crate::ast::ContextDecl,
    rename_map: &HashMap<String, String>,
) -> crate::ast::ContextDecl {
    let type_params = HashSet::new();
    context.name = rename_name(context.name, rename_map);
    for field in &mut context.fields {
        field.ty = rename_type(field.ty.clone(), rename_map, &type_params);
    }
    context
}

fn rename_bind_decl_top(mut binding: BindDecl, rename_map: &HashMap<String, String>) -> BindDecl {
    let type_params = HashSet::new();
    let mut bound = HashSet::new();
    binding.type_annotation = binding
        .type_annotation
        .map(|ty| rename_type(ty, rename_map, &type_params));
    binding.value = Box::new(rename_expr(
        *binding.value,
        rename_map,
        &type_params,
        &bound,
    ));
    if let Pattern::Ident(name) = binding.pattern {
        binding.pattern = Pattern::Ident(rename_name(name, rename_map));
    }
    collect_pattern_bindings(&binding.pattern, &mut bound);
    binding
}

fn rename_bind_decl_local(
    mut binding: BindDecl,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> BindDecl {
    binding.type_annotation = binding
        .type_annotation
        .map(|ty| rename_type(ty, rename_map, type_params));
    binding.value = Box::new(rename_expr(*binding.value, rename_map, type_params, bound));
    binding.pattern = rename_pattern_type_names(binding.pattern, rename_map, type_params);
    binding
}

fn rename_block_expr(
    mut block: BlockExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &mut HashSet<String>,
) -> BlockExpr {
    let mut statements = Vec::new();
    for stmt in block.statements {
        match stmt {
            Stmt::Binding(binding) => {
                let binding = rename_bind_decl_local(binding, rename_map, type_params, bound);
                collect_pattern_bindings(&binding.pattern, bound);
                statements.push(Stmt::Binding(binding));
            }
            Stmt::Assignment(assignment) => {
                statements.push(Stmt::Assignment(rename_assignment_stmt(
                    assignment,
                    rename_map,
                    type_params,
                    bound,
                )));
            }
            Stmt::Expr(expr) => {
                statements.push(Stmt::Expr(Box::new(rename_expr(
                    *expr,
                    rename_map,
                    type_params,
                    bound,
                ))));
            }
        }
    }

    block.statements = statements;
    block.expr = block
        .expr
        .map(|expr| Box::new(rename_expr(*expr, rename_map, type_params, bound)));
    block
}

fn rename_assignment_stmt(
    mut assignment: AssignStmt,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> AssignStmt {
    if !bound.contains(&assignment.name) {
        assignment.name = rename_name(assignment.name, rename_map);
    }
    assignment.value = Box::new(rename_expr(
        *assignment.value,
        rename_map,
        type_params,
        bound,
    ));
    assignment
}

fn rename_expr(
    expr: Expr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> Expr {
    match expr.kind {
        ExprKind::Ident(name) if bound.contains(&name) => Expr::new(ExprKind::Ident(name)),
        ExprKind::Ident(name) => Expr::new(ExprKind::Ident(rename_name(name, rename_map))),
        ExprKind::VariantRef(mut path) => {
            path.enum_name = rename_name(path.enum_name, rename_map);
            Expr::new(ExprKind::VariantRef(path))
        }
        ExprKind::RecordLit(record) => Expr::new(ExprKind::RecordLit(rename_record_lit(
            record,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::Clone(clone_expr) => Expr::new(ExprKind::Clone(rename_clone_expr(
            clone_expr,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::Freeze(expr) => Expr::new(ExprKind::Freeze(Box::new(rename_expr(
            *expr,
            rename_map,
            type_params,
            bound,
        )))),
        ExprKind::PrototypeClone(proto) => Expr::new(ExprKind::PrototypeClone(
            rename_prototype_clone_expr(proto, rename_map, type_params, bound),
        )),
        ExprKind::Then(then_expr) => Expr::new(ExprKind::Then(rename_then_expr(
            then_expr,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::While(while_expr) => Expr::new(ExprKind::While(rename_while_expr(
            while_expr,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::Match(match_expr) => Expr::new(ExprKind::Match(rename_match_expr(
            match_expr,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::Call(call) => Expr::new(ExprKind::Call(rename_call_expr(
            call,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::Binary(binary) => Expr::new(ExprKind::Binary(crate::ast::BinaryExpr {
            left: Box::new(rename_expr(*binary.left, rename_map, type_params, bound)),
            op: binary.op,
            right: Box::new(rename_expr(*binary.right, rename_map, type_params, bound)),
        })),
        ExprKind::Unary(unary) => Expr::new(ExprKind::Unary(crate::ast::UnaryExpr {
            op: unary.op,
            expr: Box::new(rename_expr(*unary.expr, rename_map, type_params, bound)),
        })),
        ExprKind::Pipe(pipe) => Expr::new(ExprKind::Pipe(rename_pipe_expr(
            pipe,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::With(with_expr) => Expr::new(ExprKind::With(rename_with_expr(
            with_expr,
            rename_map,
            type_params,
            bound,
        ))),
        ExprKind::WithLifetime(with_lifetime) => Expr::new(ExprKind::WithLifetime(
            rename_with_lifetime_expr(with_lifetime, rename_map, type_params, bound),
        )),
        ExprKind::Block(block) => {
            let mut block_bound = bound.clone();
            Expr::new(ExprKind::Block(rename_block_expr(
                block,
                rename_map,
                type_params,
                &mut block_bound,
            )))
        }
        ExprKind::FieldAccess(expr, field) => Expr::new(ExprKind::FieldAccess(
            Box::new(rename_expr(*expr, rename_map, type_params, bound)),
            field,
        )),
        ExprKind::ListLit(elements) => Expr::new(ExprKind::ListLit(
            elements
                .into_iter()
                .map(|expr| Box::new(rename_expr(*expr, rename_map, type_params, bound)))
                .collect(),
        )),
        ExprKind::ArrayLit(elements) => Expr::new(ExprKind::ArrayLit(
            elements
                .into_iter()
                .map(|expr| Box::new(rename_expr(*expr, rename_map, type_params, bound)))
                .collect(),
        )),
        ExprKind::Lambda(lambda) => {
            let mut lambda_bound = bound.clone();
            for param in &lambda.params {
                lambda_bound.insert(param.name.clone());
            }
            Expr::new(ExprKind::Lambda(crate::ast::LambdaExpr {
                params: lambda.params,
                body: Box::new(rename_expr(
                    *lambda.body,
                    rename_map,
                    type_params,
                    &lambda_bound,
                )),
            }))
        }
        ExprKind::Await(expr) => Expr::new(ExprKind::Await(Box::new(rename_expr(
            *expr,
            rename_map,
            type_params,
            bound,
        )))),
        ExprKind::Spawn(expr) => Expr::new(ExprKind::Spawn(Box::new(rename_expr(
            *expr,
            rename_map,
            type_params,
            bound,
        )))),
        ExprKind::Cast(cast) => Expr::new(ExprKind::Cast(crate::ast::CastExpr {
            expr: Box::new(rename_expr(*cast.expr, rename_map, type_params, bound)),
            target: cast.target,
        })),
        ExprKind::RangeLit(range) => Expr::new(ExprKind::RangeLit(crate::ast::RangeLit {
            start: Box::new(rename_expr(*range.start, rename_map, type_params, bound)),
            end: Box::new(rename_expr(*range.end, rename_map, type_params, bound)),
        })),
        literal => Expr::new(literal),
    }
}

fn rename_record_lit(
    mut record: RecordLit,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> RecordLit {
    record.name = rename_name(record.name, rename_map);
    record.fields = record
        .fields
        .into_iter()
        .map(|field| match field {
            FieldInit::Field { name, value } => FieldInit::Field {
                name,
                value: Box::new(rename_expr(*value, rename_map, type_params, bound)),
            },
            FieldInit::Spread(expr) => {
                FieldInit::Spread(Box::new(rename_expr(*expr, rename_map, type_params, bound)))
            }
        })
        .collect();
    record
}

fn rename_clone_expr(
    mut clone_expr: CloneExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> CloneExpr {
    clone_expr.base = Box::new(rename_expr(
        *clone_expr.base,
        rename_map,
        type_params,
        bound,
    ));
    clone_expr.updates = rename_record_lit(clone_expr.updates, rename_map, type_params, bound);
    clone_expr
}

fn rename_prototype_clone_expr(
    mut proto: PrototypeCloneExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> PrototypeCloneExpr {
    proto.base = rename_name(proto.base, rename_map);
    proto.updates = rename_record_lit(proto.updates, rename_map, type_params, bound);
    proto
}

fn rename_then_expr(
    mut then_expr: ThenExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> ThenExpr {
    then_expr.condition = Box::new(rename_expr(
        *then_expr.condition,
        rename_map,
        type_params,
        bound,
    ));
    let mut then_bound = bound.clone();
    then_expr.then_block = rename_block_expr(
        then_expr.then_block,
        rename_map,
        type_params,
        &mut then_bound,
    );
    then_expr.else_ifs = then_expr
        .else_ifs
        .into_iter()
        .map(|(condition, block)| {
            let mut branch_bound = bound.clone();
            (
                Box::new(rename_expr(*condition, rename_map, type_params, bound)),
                rename_block_expr(block, rename_map, type_params, &mut branch_bound),
            )
        })
        .collect();
    then_expr.else_block = then_expr.else_block.map(|block| {
        let mut else_bound = bound.clone();
        rename_block_expr(block, rename_map, type_params, &mut else_bound)
    });
    then_expr
}

fn rename_while_expr(
    mut while_expr: WhileExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> WhileExpr {
    while_expr.condition = Box::new(rename_expr(
        *while_expr.condition,
        rename_map,
        type_params,
        bound,
    ));
    let mut body_bound = bound.clone();
    while_expr.body = rename_block_expr(while_expr.body, rename_map, type_params, &mut body_bound);
    while_expr
}

fn rename_match_expr(
    mut match_expr: MatchExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> MatchExpr {
    match_expr.expr = Box::new(rename_expr(
        *match_expr.expr,
        rename_map,
        type_params,
        bound,
    ));
    match_expr.arms = match_expr
        .arms
        .into_iter()
        .map(|arm| rename_match_arm(arm, rename_map, type_params, bound))
        .collect();
    match_expr
}

fn rename_match_arm(
    mut arm: MatchArm,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> MatchArm {
    arm.pattern = rename_pattern_type_names(arm.pattern, rename_map, type_params);
    let mut arm_bound = bound.clone();
    collect_pattern_bindings(&arm.pattern, &mut arm_bound);
    arm.body = rename_block_expr(arm.body, rename_map, type_params, &mut arm_bound);
    arm
}

fn rename_call_expr(
    mut call: CallExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> CallExpr {
    call.function = Box::new(rename_expr(*call.function, rename_map, type_params, bound));
    call.args = call
        .args
        .into_iter()
        .map(|arg| Box::new(rename_expr(*arg, rename_map, type_params, bound)))
        .collect();
    call
}

fn rename_pipe_expr(
    mut pipe: PipeExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> PipeExpr {
    pipe.expr = Box::new(rename_expr(*pipe.expr, rename_map, type_params, bound));
    pipe.target = match pipe.target {
        PipeTarget::Ident(name) if bound.contains(&name) => PipeTarget::Ident(name),
        PipeTarget::Ident(name) => PipeTarget::Ident(rename_name(name, rename_map)),
        PipeTarget::Expr(expr) => {
            PipeTarget::Expr(Box::new(rename_expr(*expr, rename_map, type_params, bound)))
        }
    };
    pipe
}

fn rename_with_expr(
    mut with_expr: WithExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> WithExpr {
    with_expr.context_name = rename_name(with_expr.context_name, rename_map);
    with_expr.bindings = with_expr
        .bindings
        .into_iter()
        .map(|binding| match binding {
            FieldInit::Field { name, value } => FieldInit::Field {
                name,
                value: Box::new(rename_expr(*value, rename_map, type_params, bound)),
            },
            FieldInit::Spread(expr) => {
                FieldInit::Spread(Box::new(rename_expr(*expr, rename_map, type_params, bound)))
            }
        })
        .collect();
    let mut body_bound = bound.clone();
    with_expr.body = rename_block_expr(with_expr.body, rename_map, type_params, &mut body_bound);
    with_expr
}

fn rename_with_lifetime_expr(
    mut with_lifetime: WithLifetimeExpr,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
    bound: &HashSet<String>,
) -> WithLifetimeExpr {
    let mut body_bound = bound.clone();
    with_lifetime.body =
        rename_block_expr(with_lifetime.body, rename_map, type_params, &mut body_bound);
    with_lifetime
}

fn rename_pattern_type_names(
    pattern: Pattern,
    rename_map: &HashMap<String, String>,
    type_params: &HashSet<String>,
) -> Pattern {
    match pattern {
        Pattern::Record(name, fields) => Pattern::Record(
            rename_name(name, rename_map),
            fields
                .into_iter()
                .map(|(field, pattern)| {
                    (
                        field,
                        rename_pattern_type_names(pattern, rename_map, type_params),
                    )
                })
                .collect(),
        ),
        Pattern::RecordDestruct {
            type_name,
            fields,
            rest,
        } => Pattern::RecordDestruct {
            type_name: rename_type(Type::Named(type_name), rename_map, type_params).to_string(),
            fields: fields
                .into_iter()
                .map(|(field, pattern)| {
                    (
                        field,
                        rename_pattern_type_names(pattern, rename_map, type_params),
                    )
                })
                .collect(),
            rest,
        },
        Pattern::Some(pattern) => Pattern::Some(Box::new(rename_pattern_type_names(
            *pattern,
            rename_map,
            type_params,
        ))),
        Pattern::Ok(pattern) => Pattern::Ok(Box::new(rename_pattern_type_names(
            *pattern,
            rename_map,
            type_params,
        ))),
        Pattern::Err(pattern) => Pattern::Err(Box::new(rename_pattern_type_names(
            *pattern,
            rename_map,
            type_params,
        ))),
        Pattern::EnumVariant {
            enum_name,
            variant_name,
            payload,
        } => Pattern::EnumVariant {
            enum_name: rename_name(enum_name, rename_map),
            variant_name,
            payload: payload.map(|pattern| {
                Box::new(rename_pattern_type_names(*pattern, rename_map, type_params))
            }),
        },
        Pattern::ListCons(head, tail) => Pattern::ListCons(
            Box::new(rename_pattern_type_names(*head, rename_map, type_params)),
            Box::new(rename_pattern_type_names(*tail, rename_map, type_params)),
        ),
        Pattern::ListExact(patterns) => Pattern::ListExact(
            patterns
                .into_iter()
                .map(|pattern| {
                    Box::new(rename_pattern_type_names(*pattern, rename_map, type_params))
                })
                .collect(),
        ),
        other => other,
    }
}

fn collect_pattern_bindings(pattern: &Pattern, bindings: &mut HashSet<String>) {
    match pattern {
        Pattern::Ident(name) => {
            bindings.insert(name.clone());
        }
        Pattern::Record(_, fields) => {
            for (_, pattern) in fields {
                collect_pattern_bindings(pattern, bindings);
            }
        }
        Pattern::RecordDestruct { fields, rest, .. } => {
            for (_, pattern) in fields {
                collect_pattern_bindings(pattern, bindings);
            }
            if let Some(rest) = rest {
                bindings.insert(rest.clone());
            }
        }
        Pattern::Some(pattern) | Pattern::Ok(pattern) | Pattern::Err(pattern) => {
            collect_pattern_bindings(pattern, bindings);
        }
        Pattern::EnumVariant {
            payload: Some(pattern),
            ..
        } => collect_pattern_bindings(pattern, bindings),
        Pattern::EnumVariant { payload: None, .. } => {}
        Pattern::ListCons(head, tail) => {
            collect_pattern_bindings(head, bindings);
            collect_pattern_bindings(tail, bindings);
        }
        Pattern::ListExact(patterns) => {
            for pattern in patterns {
                collect_pattern_bindings(pattern, bindings);
            }
        }
        Pattern::Wildcard | Pattern::Literal(_) | Pattern::None | Pattern::EmptyList => {}
    }
}

fn get_top_decl_name_for_collision(decl: &TopDecl) -> Result<Option<String>> {
    use crate::ast::Pattern;

    match decl {
        TopDecl::Export(export_decl) => get_top_decl_name_for_collision(&export_decl.item),
        TopDecl::Binding(bind) => match &bind.pattern {
            Pattern::Ident(name) => Ok(Some(name.clone())),
            _ => Ok(None),
        },
        TopDecl::Takes(_) => Ok(None),
        _ => Ok(Some(get_decl_name(decl)?)),
    }
}

fn get_top_decl_emit_key(decl: &TopDecl) -> Result<Option<String>> {
    match decl {
        TopDecl::Export(export_decl) => get_top_decl_emit_key(&export_decl.item),
        TopDecl::Impl(impl_block) => Ok(Some(format!(
            "impl:{}:{:?}",
            impl_block.target, impl_block.functions
        ))),
        TopDecl::Takes(takes) => Ok(Some(format!("takes:{}:{}", takes.target, takes.form_name))),
        other => get_top_decl_name_for_collision(other)
            .map(|name| name.map(|name| format!("decl:{}", name))),
    }
}

fn get_decl_name(decl: &TopDecl) -> Result<String> {
    use crate::ast::Pattern;
    match decl {
        TopDecl::Function(fun) => Ok(fun.name.clone()),
        TopDecl::Record(rec) => Ok(rec.name.clone()),
        TopDecl::Enum(enum_decl) => Ok(enum_decl.name.clone()),
        TopDecl::Form(form) => Ok(form.name.clone()),
        TopDecl::Context(ctx) => Ok(ctx.name.clone()),
        TopDecl::Binding(bind) => {
            // Complex top-level binding exports need a public naming design before
            // they can be imported predictably.
            match &bind.pattern {
                Pattern::Ident(name) => Ok(name.clone()),
                _ => bail!(
                    "Complex top-level binding exports are not supported yet; export a named value instead"
                ),
            }
        }
        TopDecl::Impl(impl_block) => Ok(impl_block.target.clone()),
        TopDecl::Takes(_) => bail!("Takes declarations do not introduce a declaration name"),
        TopDecl::Export(_) => bail!("Nested exports are not allowed"),
    }
}
