use restrict_lang::ast::{
    BlockExpr, Expr, ExprKind, FunDecl, Pattern, PipeTarget, Program, Stmt, TopDecl,
};
use restrict_lang::diagnostics::{
    format_lex_error, format_parse_error, format_release_surface_error, format_type_error,
    format_unparsed_input,
};
use restrict_lang::formatter::format_source;
use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::module::{resolve_program_imports_for_file_with_package_roots, ModuleResolver};
use restrict_lang::{
    check_release_surface, lex, parse_program, HostAbiProfile, TypeChecker, WasmCodeGen,
    WasmOptimizationLevel, WasmTargetProfile,
};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[cfg(not(target_arch = "wasm32"))]
use restrict_lang::lsp;

const BIN_NAME: &str = env!("CARGO_PKG_NAME");

fn usage_text() -> String {
    format!(
        "\
Usage: {BIN_NAME} [OPTIONS] <source_file> [output_file]
Options:
  --version     Show compiler version
  --check       Check imports, types, and the selected host ABI surface without code generation
  --host-abi <PROFILE>
                Select host ABI profile: v0.0.1 (default) or flat-record-v1
  --target <PROFILE>
                Select Wasm target profile: wasip1 (default) or wasm-core
  --emit <FORMAT>
                Select output format: wat (default) or wasm
  --arena-bytes <BYTES>
                Reserve BYTES per compiler-managed arena (default: 4096)
  --release     Remove unreachable generated functions and unused runtime declarations
  --instrument-memory
                Export arena peak, live-byte, allocation, and reset metrics
  --module-root <ALIAS=DIR>
                Mount a package src directory under a source import namespace; repeatable
  --ast         Show AST only (no compilation)
  --fmt         Format source file(s) in-place (combine with --check to verify formatting)
  --verbose     Show lexing, parsing, and codegen progress details
  --lsp         Start Language Server Protocol mode
  --help        Show this help message
"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmitFormat {
    Wat,
    Wasm,
}

impl FromStr for EmitFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wat" => Ok(Self::Wat),
            "wasm" => Ok(Self::Wasm),
            _ => Err(format!(
                "Unknown output format '{value}'; expected 'wat' or 'wasm'"
            )),
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprint!("{}", usage_text());
        std::process::exit(1);
    }

    // Parse command line arguments
    let mut check_only = false;
    let mut fmt_mode = false;
    let mut show_ast = false;
    let mut lsp_mode = false;
    let mut verbose = false;
    let mut host_abi_profile = HostAbiProfile::V001Scalar;
    let mut target_profile = WasmTargetProfile::WasiP1;
    let mut emit_format = EmitFormat::Wat;
    let mut arena_size_bytes = 4096u32;
    let mut optimization_level = WasmOptimizationLevel::None;
    let mut instrument_memory = false;
    let mut package_roots = Vec::new();
    let mut source_file = String::new();
    let mut extra_source_files = Vec::new();
    let mut output_file = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                println!("{BIN_NAME} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--check" => check_only = true,
            "--host-abi" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--host-abi requires a value; expected 'v0.0.1' or 'flat-record-v1'");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--host-abi requires a value; expected 'v0.0.1' or 'flat-record-v1'");
                    std::process::exit(1);
                }
                host_abi_profile = match value.parse() {
                    Ok(profile) => profile,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                };
                i += 1;
            }
            "--target" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--target requires a value; expected 'wasm-core' or 'wasip1'");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--target requires a value; expected 'wasm-core' or 'wasip1'");
                    std::process::exit(1);
                }
                target_profile = match value.parse() {
                    Ok(profile) => profile,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                };
                i += 1;
            }
            "--emit" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--emit requires a value; expected 'wat' or 'wasm'");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--emit requires a value; expected 'wat' or 'wasm'");
                    std::process::exit(1);
                }
                emit_format = match value.parse() {
                    Ok(format) => format,
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                };
                i += 1;
            }
            "--arena-bytes" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--arena-bytes requires a positive integer");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--arena-bytes requires a positive integer");
                    std::process::exit(1);
                }
                arena_size_bytes = match value.parse::<u32>() {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        eprintln!("--arena-bytes requires a positive integer");
                        std::process::exit(1);
                    }
                };
                i += 1;
            }
            "--release" => optimization_level = WasmOptimizationLevel::Release,
            "--instrument-memory" => instrument_memory = true,
            "--module-root" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("--module-root requires ALIAS=DIR");
                    std::process::exit(1);
                };
                if value.starts_with("--") {
                    eprintln!("--module-root requires ALIAS=DIR");
                    std::process::exit(1);
                }
                match parse_module_root_spec(value) {
                    Ok(root) => package_roots.push(root),
                    Err(error) => {
                        eprintln!("{error}");
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
            "--ast" => show_ast = true,
            "--fmt" => fmt_mode = true,
            "--verbose" => verbose = true,
            "--lsp" => lsp_mode = true,
            "--help" => {
                print!("{}", usage_text());
                std::process::exit(0);
            }
            arg => {
                if source_file.is_empty() {
                    source_file = arg.to_string();
                } else if fmt_mode {
                    extra_source_files.push(arg.to_string());
                } else if output_file.is_none() {
                    output_file = Some(arg.to_string());
                } else {
                    eprintln!("Too many arguments");
                    std::process::exit(1);
                }
            }
        }
        i += 1;
    }

    if fmt_mode {
        if source_file.is_empty() {
            eprintln!("No source file specified");
            std::process::exit(1);
        }
        let mut files = Vec::with_capacity(1 + extra_source_files.len());
        files.push(source_file);
        files.extend(extra_source_files);
        run_fmt_cli(&files, check_only);
        return;
    }

    if lsp_mode {
        #[cfg(not(target_arch = "wasm32"))]
        {
            lsp::start_lsp_server().await;
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            eprintln!("Language Server Protocol mode not supported on WASM");
            std::process::exit(1);
        }
    }

    if source_file.is_empty() {
        eprintln!("No source file specified");
        std::process::exit(1);
    }

    let filename = &source_file;
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file {}: {}", filename, e);
            std::process::exit(1);
        }
    };

    // Lex the source
    if verbose && !show_ast {
        println!("=== Lexing ===");
    }
    let _tokens = match lex(&source) {
        Ok((remaining, tokens)) => {
            if !remaining.is_empty() && verbose && !show_ast {
                eprintln!("Warning: Lexer unparsed input remaining: {:?}", remaining);
                eprintln!("This might indicate a lexer issue.");
            }
            if verbose && !show_ast {
                println!("Tokens: {:?}", tokens);
            }
            tokens
        }
        Err(e) => {
            eprintln!("{}", format_lex_error(&source, e));
            std::process::exit(1);
        }
    };

    // Parse the source
    if verbose && !show_ast {
        println!("\n=== Parsing ===");
    }
    let ast = match parse_program(&source) {
        Ok((remaining, ast)) => {
            if !remaining.trim().is_empty() {
                eprintln!("{}", format_unparsed_input(&source, remaining));
                std::process::exit(1);
            }
            if show_ast {
                println!("{:#?}", ast);
                return; // Exit after showing AST
            }
            if verbose {
                println!("AST: {:#?}", ast);
            }
            ast
        }
        Err(e) => {
            eprintln!("{}", format_parse_error(&source, e));
            std::process::exit(1);
        }
    };

    let ast = match resolve_cli_imports(ast, Path::new(filename), &package_roots) {
        Ok(resolved) => resolved,
        Err(e) => {
            eprintln!("Import resolution error: {}", e);
            std::process::exit(1);
        }
    };

    // Type check
    if verbose {
        println!("\n=== Type Checking ===");
    }
    let mut type_checker = TypeChecker::new();
    match type_checker.check_program(&ast) {
        Ok(()) => {
            if let Err(e) = check_release_surface(&ast, &type_checker, host_abi_profile) {
                eprintln!("{}", format_release_surface_error(&source, &e));
                std::process::exit(1);
            }
            if let Err(e) = validate_target_profile(&ast, target_profile) {
                eprintln!("Target profile error: {}", e);
                std::process::exit(1);
            }
            if check_only {
                println!("OK {}", filename);
                return;
            }
            if verbose {
                println!("Type checking passed!");
            }
        }
        Err(e) => {
            eprintln!("{}", format_type_error(&source, &e));
            std::process::exit(1);
        }
    }

    let checked_ir = match build_checked_ir(&ast, &type_checker) {
        Ok(checked_ir) => checked_ir,
        Err(_) => {
            eprintln!("Code generation error: Invalid checked compiler state: construction failed");
            std::process::exit(1);
        }
    };

    // Generate WASM
    if verbose {
        println!("\n=== WASM Code Generation ===");
    }
    let mut codegen = match WasmCodeGen::with_host_abi_profile(host_abi_profile)
        .with_target_profile(target_profile)
        .with_optimization_level(optimization_level)
        .with_memory_instrumentation(instrument_memory)
        .with_arena_size_bytes(arena_size_bytes)
    {
        Ok(codegen) => codegen,
        Err(error) => {
            eprintln!("Code generation configuration error: {error}");
            std::process::exit(1);
        }
    };
    let wat = match codegen.generate_checked(&ast, &checked_ir) {
        Ok(wat) => {
            if verbose {
                println!("WASM generation successful!");
            }
            wat
        }
        Err(e) => {
            eprintln!("Code generation error: {}", e);
            std::process::exit(1);
        }
    };
    if verbose {
        if let Some(report) = codegen.optimization_report() {
            println!(
                "Release optimization removed {} functions, {} function imports, {} types, {} globals, {} tables, and {} element segments",
                report.removed_functions,
                report.removed_function_imports,
                report.removed_types,
                report.removed_globals,
                report.removed_tables,
                report.removed_elements,
            );
        }
    }

    // Write output
    let output_filename = output_file.unwrap_or_else(|| {
        Path::new(filename)
            .with_extension(match emit_format {
                EmitFormat::Wat => "wat",
                EmitFormat::Wasm => "wasm",
            })
            .to_str()
            .unwrap()
            .to_string()
    });

    let output = match emit_format {
        EmitFormat::Wat => wat.into_bytes(),
        EmitFormat::Wasm => {
            let wasm = match wat::parse_str(&wat) {
                Ok(wasm) => wasm,
                Err(error) => {
                    eprintln!("WebAssembly encoding error: {error}");
                    std::process::exit(1);
                }
            };
            if let Err(error) = wasmparser::Validator::new().validate_all(&wasm) {
                eprintln!("WebAssembly validation error: {error}");
                std::process::exit(1);
            }
            wasm
        }
    };

    match fs::write(&output_filename, output) {
        Ok(()) => {
            println!("\n✓ Successfully compiled to {}", output_filename);
        }
        Err(e) => {
            eprintln!("Error writing output file {}: {}", output_filename, e);
            std::process::exit(1);
        }
    }
}

fn parse_module_root_spec(value: &str) -> Result<(String, PathBuf), String> {
    let Some((namespace, source_dir)) = value.split_once('=') else {
        return Err(format!(
            "Invalid --module-root '{}': expected ALIAS=DIR",
            value
        ));
    };
    if namespace.is_empty() || source_dir.is_empty() {
        return Err(format!(
            "Invalid --module-root '{}': alias and directory must both be non-empty",
            value
        ));
    }

    Ok((namespace.to_string(), source_dir.into()))
}

fn resolve_cli_imports(
    mut ast: Program,
    source_file: &Path,
    package_roots: &[(String, PathBuf)],
) -> anyhow::Result<Program> {
    let base_dir = source_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let project_root = package_roots
        .iter()
        .find(|(namespace, _)| namespace == "project")
        .map(|(_, dir)| dir);

    let project_overlaps_entry_dir = match (project_root, fs::canonicalize(base_dir)) {
        (Some(project_dir), Ok(canonical_base)) => fs::canonicalize(project_dir)
            .ok()
            .filter(|canonical_project| {
                canonical_base.starts_with(canonical_project)
                    || canonical_project.starts_with(&canonical_base)
            })
            .map(|canonical_project| (canonical_base, canonical_project)),
        _ => None,
    };

    let Some((canonical_base, canonical_project)) = project_overlaps_entry_dir else {
        return resolve_program_imports_for_file_with_package_roots(
            ast,
            source_file,
            package_roots,
        );
    };

    let mut resolver = ModuleResolver::new();
    for (namespace, source_dir) in package_roots {
        resolver.add_package_root(namespace.clone(), source_dir.clone())?;
    }

    if ast.imports.is_empty() {
        return Ok(ast);
    }

    if canonical_base == canonical_project {
        for import in &mut ast.imports {
            let first = import.module_path.first().map(String::as_str);
            let is_known_root = first.is_some_and(|name| {
                name == "std"
                    || name == "project"
                    || package_roots.iter().any(|(alias, _)| alias == name)
            });
            if !is_known_root {
                import.module_path.insert(0, "project".to_string());
            }
        }
    }

    resolver.resolve_program_imports(ast)
}

fn validate_target_profile(
    program: &Program,
    target_profile: WasmTargetProfile,
) -> Result<(), String> {
    if target_profile != WasmTargetProfile::WasmCore {
        return Ok(());
    }

    let mut scopes = vec![HashSet::new()];
    for decl in &program.declarations {
        let inner = unwrap_export_decl(decl);
        match inner {
            TopDecl::Function(func) => {
                scopes[0].insert(func.name.clone());
            }
            TopDecl::Binding(bind) => {
                collect_pattern_bindings(&bind.pattern, &mut scopes[0]);
            }
            _ => {}
        }
    }

    for decl in &program.declarations {
        check_top_decl_for_wasm_core(decl, &mut scopes)?;
    }
    Ok(())
}

fn unwrap_export_decl(decl: &TopDecl) -> &TopDecl {
    match decl {
        TopDecl::Export(export) => unwrap_export_decl(export.item.as_ref()),
        other => other,
    }
}

fn check_top_decl_for_wasm_core(
    decl: &TopDecl,
    scopes: &mut Vec<HashSet<String>>,
) -> Result<(), String> {
    match unwrap_export_decl(decl) {
        TopDecl::Function(func) => check_fun_decl_for_wasm_core(func, scopes),
        TopDecl::Binding(bind) => check_expr_for_wasm_core(&bind.value, scopes),
        TopDecl::Impl(impl_block) => {
            for func in &impl_block.functions {
                check_fun_decl_for_wasm_core(func, scopes)?;
            }
            Ok(())
        }
        TopDecl::Takes(takes_decl) => {
            for func in &takes_decl.functions {
                check_fun_decl_for_wasm_core(func, scopes)?;
            }
            Ok(())
        }
        TopDecl::Record(_)
        | TopDecl::Enum(_)
        | TopDecl::Form(_)
        | TopDecl::Context(_)
        | TopDecl::Export(_) => Ok(()),
    }
}

fn check_fun_decl_for_wasm_core(
    func: &FunDecl,
    scopes: &mut Vec<HashSet<String>>,
) -> Result<(), String> {
    let mut fn_scope = HashSet::new();
    for param in &func.params {
        fn_scope.insert(param.name.clone());
    }
    scopes.push(fn_scope);
    let result = check_block_for_wasm_core(&func.body, scopes);
    scopes.pop();
    result
}

fn check_block_for_wasm_core(
    block: &BlockExpr,
    scopes: &mut Vec<HashSet<String>>,
) -> Result<(), String> {
    scopes.push(HashSet::new());
    for stmt in &block.statements {
        match stmt {
            Stmt::Binding(bind) => {
                if let Err(err) = check_expr_for_wasm_core(&bind.value, scopes) {
                    scopes.pop();
                    return Err(err);
                }
                if let Some(current) = scopes.last_mut() {
                    collect_pattern_bindings(&bind.pattern, current);
                }
            }
            Stmt::Assignment(assign) => {
                if let Err(err) = check_expr_for_wasm_core(&assign.value, scopes) {
                    scopes.pop();
                    return Err(err);
                }
            }
            Stmt::Expr(expr) => {
                if let Err(err) = check_expr_for_wasm_core(expr, scopes) {
                    scopes.pop();
                    return Err(err);
                }
            }
        }
    }
    let result = match &block.expr {
        Some(expr) => check_expr_for_wasm_core(expr, scopes),
        None => Ok(()),
    };
    scopes.pop();
    result
}

fn check_expr_for_wasm_core(expr: &Expr, scopes: &mut Vec<HashSet<String>>) -> Result<(), String> {
    match &expr.kind {
        ExprKind::Ident(name) => reject_if_host_io_builtin(name, scopes),
        ExprKind::Call(call) => {
            for arg in &call.args {
                check_expr_for_wasm_core(arg, scopes)?;
            }
            if let ExprKind::Ident(name) = &call.function.kind {
                reject_if_host_io_builtin(name, scopes)
            } else {
                check_expr_for_wasm_core(&call.function, scopes)
            }
        }
        ExprKind::Pipe(pipe) => {
            check_expr_for_wasm_core(&pipe.expr, scopes)?;
            match &pipe.target {
                PipeTarget::Ident(name) => reject_if_host_io_builtin(name, scopes),
                PipeTarget::Expr(target) => check_expr_for_wasm_core(target, scopes),
            }
        }
        ExprKind::Block(block) => check_block_for_wasm_core(block, scopes),
        ExprKind::Lambda(lambda) => {
            let mut lambda_scope = HashSet::new();
            for param in &lambda.params {
                lambda_scope.insert(param.name.clone());
            }
            if lambda.implicit_focus {
                lambda_scope.insert("it".to_string());
            }
            scopes.push(lambda_scope);
            let result = check_expr_for_wasm_core(&lambda.body, scopes);
            scopes.pop();
            result
        }
        ExprKind::Then(then_expr) => {
            check_expr_for_wasm_core(&then_expr.condition, scopes)?;
            check_block_for_wasm_core(&then_expr.then_block, scopes)?;
            for (cond, block) in &then_expr.else_ifs {
                check_expr_for_wasm_core(cond, scopes)?;
                check_block_for_wasm_core(block, scopes)?;
            }
            if let Some(else_block) = &then_expr.else_block {
                check_block_for_wasm_core(else_block, scopes)?;
            }
            Ok(())
        }
        ExprKind::While(while_expr) => {
            check_expr_for_wasm_core(&while_expr.condition, scopes)?;
            check_block_for_wasm_core(&while_expr.body, scopes)
        }
        ExprKind::Match(match_expr) => {
            check_expr_for_wasm_core(&match_expr.expr, scopes)?;
            for arm in &match_expr.arms {
                let mut arm_scope = HashSet::new();
                collect_pattern_bindings(&arm.pattern, &mut arm_scope);
                scopes.push(arm_scope);
                let result = check_block_for_wasm_core(&arm.body, scopes);
                scopes.pop();
                result?;
            }
            Ok(())
        }
        ExprKind::RecordLit(record) => {
            for field in &record.fields {
                match field {
                    restrict_lang::ast::FieldInit::Field { value, .. }
                    | restrict_lang::ast::FieldInit::Spread(value) => {
                        check_expr_for_wasm_core(value, scopes)?;
                    }
                }
            }
            Ok(())
        }
        ExprKind::Clone(clone) => {
            check_expr_for_wasm_core(&clone.base, scopes)?;
            for field in &clone.updates.fields {
                match field {
                    restrict_lang::ast::FieldInit::Field { value, .. }
                    | restrict_lang::ast::FieldInit::Spread(value) => {
                        check_expr_for_wasm_core(value, scopes)?;
                    }
                }
            }
            Ok(())
        }
        ExprKind::Freeze(inner)
        | ExprKind::FieldAccess(inner, _)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner) => check_expr_for_wasm_core(inner, scopes),
        ExprKind::PrototypeClone(proto) => {
            for field in &proto.updates.fields {
                match field {
                    restrict_lang::ast::FieldInit::Field { value, .. }
                    | restrict_lang::ast::FieldInit::Spread(value) => {
                        check_expr_for_wasm_core(value, scopes)?;
                    }
                }
            }
            Ok(())
        }
        ExprKind::Binary(binary) => {
            check_expr_for_wasm_core(&binary.left, scopes)?;
            check_expr_for_wasm_core(&binary.right, scopes)
        }
        ExprKind::Unary(unary) => check_expr_for_wasm_core(&unary.expr, scopes),
        ExprKind::Cast(cast) => check_expr_for_wasm_core(&cast.expr, scopes),
        ExprKind::With(with_expr) => {
            for binding in &with_expr.bindings {
                match binding {
                    restrict_lang::ast::FieldInit::Field { value, .. }
                    | restrict_lang::ast::FieldInit::Spread(value) => {
                        check_expr_for_wasm_core(value, scopes)?;
                    }
                }
            }
            check_block_for_wasm_core(&with_expr.body, scopes)
        }
        ExprKind::WithLifetime(with_lt) => check_block_for_wasm_core(&with_lt.body, scopes),
        ExprKind::ListLit(items) | ExprKind::ArrayLit(items) => {
            for item in items {
                check_expr_for_wasm_core(item, scopes)?;
            }
            Ok(())
        }
        ExprKind::RangeLit(range) => {
            check_expr_for_wasm_core(&range.start, scopes)?;
            check_expr_for_wasm_core(&range.end, scopes)
        }
        ExprKind::Return(Some(inner)) => check_expr_for_wasm_core(inner, scopes),
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::BoolLit(_)
        | ExprKind::Unit
        | ExprKind::VariantRef(_)
        | ExprKind::Break
        | ExprKind::Continue
        | ExprKind::Return(None) => Ok(()),
    }
}

fn collect_pattern_bindings(pattern: &Pattern, bound: &mut HashSet<String>) {
    match pattern {
        Pattern::Ident(name) => {
            bound.insert(name.clone());
        }
        Pattern::Record(_, fields) => {
            for (_, sub) in fields {
                collect_pattern_bindings(sub, bound);
            }
        }
        Pattern::RecordDestruct { fields, rest, .. } => {
            for (_, sub) in fields {
                collect_pattern_bindings(sub, bound);
            }
            if let Some(rest_name) = rest {
                bound.insert(rest_name.clone());
            }
        }
        Pattern::Some(inner) | Pattern::Ok(inner) | Pattern::Err(inner) => {
            collect_pattern_bindings(inner, bound);
        }
        Pattern::EnumVariant {
            payload: Some(inner),
            ..
        } => {
            collect_pattern_bindings(inner, bound);
        }
        Pattern::ListCons(head, tail) => {
            collect_pattern_bindings(head, bound);
            collect_pattern_bindings(tail, bound);
        }
        Pattern::ListExact(items) => {
            for item in items {
                collect_pattern_bindings(item, bound);
            }
        }
        Pattern::Wildcard
        | Pattern::Literal(_)
        | Pattern::None
        | Pattern::EnumVariant { payload: None, .. }
        | Pattern::EmptyList => {}
    }
}

fn run_fmt_cli(files: &[String], check_only: bool) {
    let mut needs_formatting = false;
    for path in files {
        let source = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", path, e);
                std::process::exit(1);
            }
        };
        let formatted = match format_source(&source) {
            Ok(formatted) => formatted,
            Err(e) => {
                eprintln!("Formatting error in {}: {}", path, e);
                std::process::exit(1);
            }
        };
        if formatted != source {
            if check_only {
                eprintln!("Would reformat {}", path);
                needs_formatting = true;
            } else if let Err(e) = fs::write(path, formatted) {
                eprintln!("Error writing file {}: {}", path, e);
                std::process::exit(1);
            } else {
                println!("Formatted {}", path);
            }
        }
    }
    if check_only && needs_formatting {
        std::process::exit(1);
    }
}

fn reject_if_host_io_builtin(name: &str, scopes: &[HashSet<String>]) -> Result<(), String> {
    let is_host_io = matches!(
        name,
        "println"
            | "print"
            | "eprint"
            | "eprintln"
            | "print_int"
            | "print_float"
            | "cli_args"
            | "env_get"
            | "stdin_read_all"
            | "exit"
    );
    if is_host_io && !scopes.iter().any(|scope| scope.contains(name)) {
        return Err(format!(
            "'{name}' requires the 'wasip1' target profile; 'wasm-core' does not import host I/O"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_core_validation_rejects_new_wasi_builtins() {
        for (name, call_expr) in [
            ("cli_args", "() cli_args"),
            ("env_get", "\"HOME\" |> env_get"),
            ("stdin_read_all", "() stdin_read_all"),
            ("exit", "1 |> exit"),
        ] {
            let source = format!("fun main: () -> () = {{ val _v = {call_expr}; () }}\n");
            let (rem, program) = parse_program(&source).expect("source should parse");
            assert!(rem.trim().is_empty());
            let err = validate_target_profile(&program, WasmTargetProfile::WasmCore)
                .expect_err("wasm-core must reject WASI builtin");
            assert!(err.contains(name), "{err}");
            assert!(err.contains("wasip1"), "{err}");
        }
    }
}
