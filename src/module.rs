use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use anyhow::{Result, Context, bail};
use crate::ast::{Program, ImportItems, TopDecl};
use crate::parser::parse_program;
use std::fs;

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
    /// Modules currently being resolved (for cycle detection)
    resolving: HashSet<Vec<String>>,
}

impl ModuleResolver {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            search_paths: vec![PathBuf::from(".")],
            resolving: HashSet::new(),
        }
    }

    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn resolve_module(&mut self, module_path: &[String]) -> Result<Vec<String>> {
        // Check if already loaded
        if self.modules.contains_key(module_path) {
            return Ok(module_path.to_vec());
        }

        // Check for circular dependency
        if self.resolving.contains(module_path) {
            bail!(
                "Circular dependency detected: {} is already being resolved.\n\
                 Import chain involves: {}",
                module_path.join("."),
                self.resolving.iter()
                    .map(|p| p.join("."))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }

        // Mark as currently resolving
        self.resolving.insert(module_path.to_vec());

        // Try to find the module file
        let file_path = self.find_module_file(module_path)?;
        
        // Load and parse the module
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read module file: {:?}", file_path))?;
        
        let (remaining, mut program) = parse_program(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse module: {:?}", e))?;
        
        if !remaining.trim().is_empty() {
            bail!("Unexpected content after module: {}", remaining);
        }
        
        // Collect imports to process later
        let imports = program.imports.clone();
        
        // Build export table
        let mut exports = HashMap::new();
        let mut regular_decls = Vec::new();
        
        for decl in program.declarations {
            match decl {
                TopDecl::Export(export_decl) => {
                    let name = get_decl_name(&export_decl.item);
                    exports.insert(name, *export_decl.item);
                }
                decl => regular_decls.push(decl),
            }
        }
        
        program.declarations = regular_decls;
        
        // Process imports BEFORE inserting into modules
        // This allows cycle detection to work properly
        for import in &imports {
            self.resolve_module(&import.module_path)?;
        }

        let module = Module {
            path: file_path,
            name: module_path.to_vec(),
            program,
            exports,
        };

        self.modules.insert(module_path.to_vec(), module);

        // Remove from resolving set (module is now fully resolved)
        self.resolving.remove(module_path);

        Ok(module_path.to_vec())
    }
    
    pub fn get_module(&self, module_path: &[String]) -> Option<&Module> {
        self.modules.get(module_path)
    }
    
    fn find_module_file(&self, module_path: &[String]) -> Result<PathBuf> {
        let relative_path = module_path.join("/") + ".rl";
        
        for search_path in &self.search_paths {
            let full_path = search_path.join(&relative_path);
            if full_path.exists() {
                return Ok(full_path);
            }
        }
        
        bail!("Module not found: {}", module_path.join("."))
    }
    
    pub fn get_imported_items(&self, module_path: &[String], items: &ImportItems) -> Result<Vec<(String, TopDecl)>> {
        let module = self.modules.get(module_path)
            .with_context(|| format!("Module not resolved: {}", module_path.join(".")))?;
        
        match items {
            ImportItems::All => {
                Ok(module.exports.iter()
                    .map(|(name, decl)| (name.clone(), decl.clone()))
                    .collect())
            }
            ImportItems::Named(names) => {
                let mut result = Vec::new();
                for name in names {
                    let decl = module.exports.get(name)
                        .with_context(|| format!("Export '{}' not found in module {}", name, module_path.join(".")))?;
                    result.push((name.clone(), decl.clone()));
                }
                Ok(result)
            }
        }
    }
}

fn get_decl_name(decl: &TopDecl) -> String {
    match decl {
        TopDecl::Function(fun) => fun.name.clone(),
        TopDecl::Record(rec) => rec.name.clone(),
        TopDecl::Context(ctx) => ctx.name.clone(),
        TopDecl::Binding(bind) => bind.name.clone(),
        TopDecl::Impl(impl_block) => impl_block.target.clone(),
        TopDecl::Export(_) => panic!("Nested exports not allowed"),
    }
}