use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

struct NonReleaseExample {
    path: &'static str,
    reason: &'static str,
}

struct ReleaseExampleCliException {
    path: &'static str,
    reason: &'static str,
}

const RELEASE_EXAMPLES: &[&str] = &[
    "examples/checkout_review.rl",
    "examples/order_pricing.rl",
    "examples/generic_inference.rl",
    "examples/deploy_gate.rl",
    "examples/incident_triage.rl",
    "examples/fulfillment_batch.rl",
    "examples/inventory_reorder.rl",
    "examples/sensor_calibration.rl",
    "examples/status_routing.rl",
    "examples/result_validation.rl",
    "examples/retry_budget.rl",
    "examples/release_readiness.rl",
    "examples/sprint_planner.rl",
    "examples/support_queue.rl",
    "examples/service_monitor.rl",
    "examples/rollout_handoff.rl",
    "examples/feature_rollout_policy.rl",
    "examples/bug_triage_board.rl",
    "examples/review_policy_factory.rl",
    "examples/change_review_gate.rl",
    "examples/calibration_pipeline.rl",
    "examples/experiment_scorecard.rl",
    "examples/release_queue_snapshot.rl",
    "examples/release_review_digest.rl",
    "examples/generic_function_value_pipeline.rl",
    "examples/release_decision_engine.rl",
    "examples/typed_impl_dispatch.rl",
    "examples/context_policy_gate.rl",
    "examples/subscription_billing.rl",
    "examples/modular_release_gate.rl",
    "examples/modular_release_import_surface.rl",
    "examples/modular_policy_context_gate.rl",
    "examples/modules/release_policy.rl",
    "examples/modules/release_scores.rl",
    "examples/modules/policy_context.rl",
    "examples/dogfood_type_inference.rl",
    "examples/dogfood_branch_callable_prefix_inference.rl",
    "examples/lambda_expected_inference.rl",
    "examples/lambda_inference.rl",
    "examples/list_example.rl",
    "examples/test_comments.rl",
    "examples/return_annotation_contract.rl",
    "examples/dogfood_ci_test_planner_inference.rl",
    "examples/dogfood_empty_branch_inference.rl",
    "examples/dogfood_inference_task_queue.rl",
    "examples/dogfood_metrics_rollup_inference.rl",
    "examples/dogfood_release_readiness_inference.rl",
    "examples/dogfood_release_patch_inference.rl",
    "examples/dogfood_result_local_inference.rl",
    "examples/dogfood_mutable_checkpoint_inference.rl",
    "examples/dogfood_slo_budget_inference.rl",
    "examples/dogfood_spec_literals_inference.rl",
    "examples/dogfood_support_escalation_inference.rl",
    "examples/dogfood_support_rotation_inference.rl",
    "examples/dogfood_array_range_window_inference.rl",
    "examples/dogfood_list_tail_inference.rl",
    "examples/dogfood_impl_dispatch_inference.rl",
    "examples/dogfood_option_container_lambda_inference.rl",
    "examples/dogfood_nested_result_list_inference.rl",
    "examples/dogfood_callable_match_inference.rl",
    "examples/dogfood_callable_codegen_handoff.rl",
    "examples/dogfood_bug_triage_board_runtime.rl",
    "examples/dogfood_generic_pipeline_runtime.rl",
    "examples/dogfood_incident_triage_runtime.rl",
    "examples/dogfood_generic_context_inference.rl",
    "examples/elegant_test.rl",
    "examples/language_tests.rl",
    "examples/pure_test.rl",
    "examples/restrict_test.rl",
    "examples/test_framework.rl",
    "examples/ultra_test.rl",
    "examples/zen_test.rl",
];

const KNOWN_EXPERIMENTAL_OR_STALE_EXAMPLES: &[NonReleaseExample] = &[
    NonReleaseExample {
        path: "examples/simple_test.rl",
        reason: "legacy parser sketch, not a release example",
    },
    NonReleaseExample {
        path: "examples/spread_destructuring_demo.rl",
        reason: "design sketch for spread forms beyond the release gate",
    },
    NonReleaseExample {
        path: "examples/std_lib.rl",
        reason: "legacy standard-library sketch, not a release example",
    },
    NonReleaseExample {
        path: "examples/field_access_patterns.rl",
        reason: "parser/type-checker sketch for field-access variants",
    },
    NonReleaseExample {
        path: "examples/form_container.rl",
        reason: "form/takes design document outside the v0.0.1 default gate",
    },
    NonReleaseExample {
        path: "examples/option_example.rl",
        reason: "legacy option sketch that has not been promoted to the release example surface",
    },
    NonReleaseExample {
        path: "examples/tat_cleanup_demo.rl",
        reason: "temporal affine type demo outside the v0.0.1 default gate",
    },
    NonReleaseExample {
        path: "examples/temporal_file_io.rl",
        reason: "temporal affine type demo outside the v0.0.1 default gate",
    },
    NonReleaseExample {
        path: "examples/temporal_simple.rl",
        reason: "temporal affine type demo outside the v0.0.1 default gate",
    },
    NonReleaseExample {
        path: "examples/test_context_temporal.rl",
        reason: "temporal context sketch outside the v0.0.1 default gate",
    },
    NonReleaseExample {
        path: "examples/dogfood_generic_export_gap.rl",
        reason:
            "v0.0.1 design gap: exported generic functions type-check but lack a concrete WASM ABI",
    },
    NonReleaseExample {
        path: "test_input.rl",
        reason: "legacy root-level smoke fixture outside the v0.0.1 release surface",
    },
    NonReleaseExample {
        path: "test_main.rl",
        reason: "legacy root-level smoke fixture outside the v0.0.1 release surface",
    },
    NonReleaseExample {
        path: "test_unit_return.rl",
        reason: "legacy root-level Unit-return smoke fixture outside the v0.0.1 release surface",
    },
];

const RELEASE_EXAMPLE_CLI_EXCEPTIONS: &[ReleaseExampleCliException] = &[
    ReleaseExampleCliException {
        path: "examples/modules/release_policy.rl",
        reason: "module leaf covered through examples/modular_release_gate.rl",
    },
    ReleaseExampleCliException {
        path: "examples/modules/release_scores.rl",
        reason: "module leaf covered transitively through examples/modular_release_gate.rl",
    },
    ReleaseExampleCliException {
        path: "examples/modules/policy_context.rl",
        reason: "module leaf covered through examples/modular_policy_context_gate.rl",
    },
];

const VSCODE_RELEASE_EXAMPLE_FILES: &[&str] = &[
    "vscode-extension/examples/hello.rl",
    "vscode-extension/examples/test.rl",
    "vscode-extension/examples/test_function.rl",
];

const VSCODE_RELEASE_SNIPPET_FILES: &[&str] = &["vscode-extension/snippets/restrict.json"];

const VSCODE_RELEASE_README_FILES: &[&str] = &["vscode-extension/README.md"];

#[test]
fn release_examples_are_explicit_and_present() {
    assert_unique("release example", RELEASE_EXAMPLES);
    assert_all_exist("release example", RELEASE_EXAMPLES);
}

#[test]
fn release_examples_use_current_syntax() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in RELEASE_EXAMPLES {
        let source = read_source(root, relative_path);
        assert_current_example_syntax(relative_path, &source);
    }
}

#[test]
#[ignore = "slow CLI release gate; run with `mise run check`"]
fn standalone_release_examples_compile_through_cli() {
    assert_unique("release example", RELEASE_EXAMPLES);
    assert_all_exist("release example", RELEASE_EXAMPLES);
    let exceptions: HashSet<_> = RELEASE_EXAMPLE_CLI_EXCEPTIONS
        .iter()
        .map(|exception| {
            assert!(
                !exception.reason.trim().is_empty(),
                "{} should explain why it is excluded from standalone CLI validation",
                exception.path
            );
            assert!(
                RELEASE_EXAMPLES.contains(&exception.path),
                "{} should be a release example before it can be a CLI exception",
                exception.path
            );
            exception.path
        })
        .collect();

    for relative_path in RELEASE_EXAMPLES.iter().copied() {
        if exceptions.contains(relative_path) {
            continue;
        }
        assert_cli_compiles(relative_path);
    }
}

#[test]
fn mise_check_task_matches_standalone_release_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mise = read_source(root, ".mise.toml");
    let task_body = mise_check_task_body(&mise);
    let task_examples = mise_check_examples(task_body);
    assert!(
        task_body.contains("cargo test --test test_release_example_hygiene standalone_release_examples_compile_through_cli -- --ignored --exact"),
        ".mise.toml tasks.check should delegate to the ignored release example CLI gate"
    );
    assert!(
        task_body.contains("cargo test --test test_release_example_hygiene vscode_release_examples_compile_through_cli -- --ignored --exact"),
        ".mise.toml tasks.check should include the ignored VS Code release example CLI gate"
    );

    let exceptions: HashSet<_> = RELEASE_EXAMPLE_CLI_EXCEPTIONS
        .iter()
        .map(|exception| exception.path)
        .collect();
    let expected = RELEASE_EXAMPLES
        .iter()
        .copied()
        .filter(|path| !exceptions.contains(path))
        .collect::<HashSet<_>>();
    let actual = task_examples
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    let mut missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    missing.sort();
    let mut stale = actual.difference(&expected).copied().collect::<Vec<_>>();
    stale.sort();

    assert!(
        task_examples.is_empty() || (missing.is_empty() && stale.is_empty()),
        ".mise.toml tasks.check should either delegate to the release example manifest test or match standalone release examples.\nmissing: {}\nstale: {}",
        missing.join(", "),
        stale.join(", ")
    );
}

#[test]
fn module_leaf_release_examples_are_covered_by_modular_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_sources = RELEASE_EXAMPLES
        .iter()
        .map(|path| (*path, read_source(root, path)))
        .collect::<Vec<_>>();

    for relative_path in RELEASE_EXAMPLES
        .iter()
        .copied()
        .filter(|path| path.starts_with("examples/modules/"))
    {
        let module_name = relative_path
            .trim_start_matches("examples/")
            .trim_end_matches(".rl")
            .replace('/', ".");
        let imported_by_release_example = release_sources
            .iter()
            .filter(|(path, _)| path != &relative_path)
            .any(|(_, source)| source.contains(&module_name));
        assert!(
            imported_by_release_example,
            "{relative_path} should be imported by another release example so module leaf release examples have CLI coverage"
        );
    }
}

#[test]
fn authoritative_specs_include_impl_release_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = read_source(root, "LANGUAGE_SPECIFICATION.md");
    let ebnf = read_source(root, "RESTRICT_LANG_EBNF.md");

    assert!(
        contains_word(&spec, "impl"),
        "LANGUAGE_SPECIFICATION.md should list impl as current release syntax"
    );
    assert!(
        spec.contains("Implementation Blocks")
            && spec.contains("(receiver) method")
            && spec.contains("(receiver, args...) method"),
        "LANGUAGE_SPECIFICATION.md should describe type-directed impl methods as OSV calls"
    );
    assert!(
        ebnf.contains("impl_decl") && ebnf.contains("\"impl\" identifier"),
        "RESTRICT_LANG_EBNF.md should include impl block grammar"
    );
}

#[test]
fn known_experimental_or_stale_examples_are_visible() {
    let known_paths = non_release_paths();

    assert_unique("known experimental or stale example", &known_paths);
    assert_disjoint(
        "release examples",
        RELEASE_EXAMPLES,
        "known experimental or stale examples",
        &known_paths,
    );
    assert_all_exist("known experimental or stale example", &known_paths);

    for example in KNOWN_EXPERIMENTAL_OR_STALE_EXAMPLES {
        assert!(
            !example.reason.trim().is_empty(),
            "{} should explain why it is outside the v0.0.1 release example gate",
            example.path
        );
    }
}

#[test]
fn repository_examples_are_all_classified() {
    let release_paths: HashSet<_> = RELEASE_EXAMPLES.iter().copied().collect();
    let non_release_paths: HashSet<_> = non_release_paths().into_iter().collect();

    for relative_path in example_restrict_sources()
        .into_iter()
        .filter(|path| path.starts_with("examples/"))
    {
        assert!(
            release_paths.contains(relative_path.as_str())
                || non_release_paths.contains(relative_path.as_str()),
            "tracked {relative_path} should be listed as a v0.0.1 release example or as explicitly stale/experimental"
        );
    }
}

#[test]
fn tracked_root_restrict_sources_are_explicitly_non_release() {
    let release_paths: HashSet<_> = RELEASE_EXAMPLES.iter().copied().collect();
    let non_release_paths: HashSet<_> = non_release_paths().into_iter().collect();

    for relative_path in tracked_restrict_sources()
        .into_iter()
        .filter(|path| Path::new(path).components().count() == 1)
    {
        assert!(
            !release_paths.contains(relative_path.as_str()),
            "{relative_path} is a tracked root-level .rl file; move it under examples/ before listing it as a release example"
        );
        assert!(
            non_release_paths.contains(relative_path.as_str()),
            "{relative_path} is a tracked root-level .rl file and must be explicitly classified so it cannot silently stay in the release surface"
        );
    }
}

#[test]
fn vscode_release_examples_use_current_syntax() {
    assert_unique("VS Code release example", VSCODE_RELEASE_EXAMPLE_FILES);
    assert_all_exist("VS Code release example", VSCODE_RELEASE_EXAMPLE_FILES);
    assert_all_exist("VS Code release snippet", VSCODE_RELEASE_SNIPPET_FILES);
    assert_all_exist("VS Code release README", VSCODE_RELEASE_README_FILES);

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in VSCODE_RELEASE_EXAMPLE_FILES
        .iter()
        .chain(VSCODE_RELEASE_SNIPPET_FILES)
    {
        let source = read_source(root, relative_path);
        assert_current_example_syntax(relative_path, &source);
    }

    for relative_path in VSCODE_RELEASE_README_FILES {
        let source = read_source(root, relative_path);
        let restrict_blocks = restrict_code_blocks(&source);
        assert!(
            !restrict_blocks.is_empty(),
            "{relative_path} should include a Restrict code block"
        );
        assert_current_example_syntax(relative_path, &restrict_blocks.join("\n"));
    }
}

#[test]
#[ignore = "slow CLI release gate; run with `mise run check`"]
fn vscode_release_examples_compile_through_cli() {
    assert_unique("VS Code release example", VSCODE_RELEASE_EXAMPLE_FILES);
    assert_all_exist("VS Code release example", VSCODE_RELEASE_EXAMPLE_FILES);

    for relative_path in VSCODE_RELEASE_EXAMPLE_FILES {
        assert_cli_compiles(relative_path);
    }
}

fn non_release_paths() -> Vec<&'static str> {
    KNOWN_EXPERIMENTAL_OR_STALE_EXAMPLES
        .iter()
        .map(|example| example.path)
        .collect()
}

fn mise_check_task_body(mise: &str) -> &str {
    let Some(task_start) = mise.find("[tasks.check]") else {
        return "";
    };
    let task_body = &mise[task_start..];
    let task_end = task_body
        .find("\n[tasks.")
        .map(|offset| task_start + offset)
        .unwrap_or(mise.len());
    &mise[task_start..task_end]
}

fn mise_check_examples(task_body: &str) -> Vec<String> {
    let mut examples = Vec::new();

    for line in task_body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("examples/") {
            let path = trimmed
                .trim_end_matches('\\')
                .trim()
                .split_whitespace()
                .next()
                .expect("example line should contain a path");
            examples.push(path.to_string());
        }
    }

    examples
}

fn assert_all_exist(label: &str, relative_paths: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative_path in relative_paths {
        let path = root.join(relative_path);
        assert!(path.is_file(), "{label} should exist: {}", path.display());
    }
}

fn assert_unique(label: &str, relative_paths: &[&str]) {
    let mut seen = HashSet::new();

    for relative_path in relative_paths {
        assert!(
            seen.insert(*relative_path),
            "{label} should only be listed once: {relative_path}"
        );
    }
}

fn assert_disjoint(left_label: &str, left_paths: &[&str], right_label: &str, right_paths: &[&str]) {
    let left: HashSet<_> = left_paths.iter().copied().collect();

    for right_path in right_paths {
        assert!(
            !left.contains(right_path),
            "{right_path} should not be listed as both {left_label} and {right_label}"
        );
    }
}

fn read_source(root: &Path, relative_path: &str) -> String {
    fs::read_to_string(root.join(relative_path))
        .unwrap_or_else(|err| panic!("failed to read {relative_path}: {err}"))
}

fn assert_cli_compiles(relative_path: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_path = root.join(relative_path);
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_{}_{}.wat",
        relative_path.replace(['/', '\\', '.'], "_"),
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{relative_path} should compile through the CLI\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let wat = fs::read_to_string(&output_path)
        .unwrap_or_else(|err| panic!("compiled WAT should be readable for {relative_path}: {err}"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("{relative_path} should generate valid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{relative_path} should generate valid Wasm: {err}\n\n{wat}");
        });

    let _ = fs::remove_file(output_path);
}

fn tracked_restrict_sources() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("git")
        .args(["ls-files", "--"])
        .current_dir(root)
        .output()
        .expect("git ls-files should run for release hygiene checks");
    let stdout = String::from_utf8(output.stdout).expect("git ls-files output should be UTF-8");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "git ls-files should succeed for release hygiene checks\nstderr:\n{stderr}"
    );

    let mut sources: Vec<_> = stdout
        .lines()
        .filter(|path| Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("rl"))
        .map(str::to_owned)
        .collect();
    sources.sort();
    sources
}

fn example_restrict_sources() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    collect_restrict_sources(root, &root.join("examples"), &mut sources);
    sources.sort();
    sources
}

fn collect_restrict_sources(root: &Path, dir: &Path, sources: &mut Vec<String>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", dir.display()));

    for entry in entries {
        let path = entry
            .unwrap_or_else(|err| panic!("entry under {} should be readable: {err}", dir.display()))
            .path();
        if path.is_dir() {
            collect_restrict_sources(root, &path, sources);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rl") {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or_else(|err| panic!("{} should be under repo root: {err}", path.display()))
            .to_string_lossy()
            .replace('\\', "/");
        sources.push(relative);
    }
}

fn restrict_code_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut active_block = Vec::new();
    let mut in_restrict_block = false;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if in_restrict_block {
            if trimmed.starts_with("```") {
                blocks.push(active_block.join("\n"));
                active_block.clear();
                in_restrict_block = false;
            } else {
                active_block.push(line);
            }
        } else if trimmed == "```restrict" {
            in_restrict_block = true;
        }
    }

    blocks
}

fn assert_current_example_syntax(label: &str, source: &str) {
    assert_no_stale_phrase(label, source, "val mut", "mut val");
    assert_no_stale_phrase(label, source, "[|", "list or array literals with [");

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;

        assert_no_stale_type_name(label, line_number, line, "Int", "Int32");
        assert_no_stale_type_name(label, line_number, line, "Bool", "Boolean");
        assert_no_stale_type_name(label, line_number, line, "Unit", "()");
        assert_no_stale_function_declaration(label, line_number, line);
        assert_no_object_style_method_call(label, line_number, line);
        assert_no_traditional_function_call(label, line_number, line);
        assert_no_future_form_syntax(label, line_number, line);
    }
}

fn assert_no_stale_phrase(label: &str, source: &str, stale: &str, replacement: &str) {
    assert!(
        !source.contains(stale),
        "{label} should use {replacement} instead of stale syntax {stale:?}"
    );
}

fn assert_no_stale_type_name(
    label: &str,
    line_number: usize,
    line: &str,
    stale: &str,
    replacement: &str,
) {
    assert!(
        !contains_word(line, stale),
        "{label}:{line_number} should use {replacement} instead of stale type {stale}"
    );
}

fn assert_no_stale_function_declaration(label: &str, line_number: usize, line: &str) {
    for (fun_index, _) in line.match_indices("fun ") {
        let after_fun = &line[fun_index + "fun ".len()..];
        let chars: Vec<_> = after_fun.chars().collect();
        let mut index = chars
            .iter()
            .position(|char_| !char_.is_whitespace())
            .unwrap_or(0);

        if chars.get(index) == Some(&'$') && chars.get(index + 1) == Some(&'{') {
            let Some(placeholder_end) = chars[index..].iter().position(|char_| *char_ == '}')
            else {
                continue;
            };
            index += placeholder_end + 1;
        } else if chars.get(index).is_some_and(|char_| is_ident_start(*char_)) {
            index = ident_end(&chars, index);
        } else {
            continue;
        }

        while chars.get(index).is_some_and(|char_| char_.is_whitespace()) {
            index += 1;
        }

        assert!(
            chars.get(index) != Some(&'='),
            "{label}:{line_number} should use `fun name: (...) = {{ ... }}` instead of `fun name =`"
        );
    }
}

fn assert_no_object_style_method_call(label: &str, line_number: usize, line: &str) {
    let chars: Vec<_> = line.chars().collect();

    for (index, char_) in chars.iter().enumerate() {
        if *char_ != '.' {
            continue;
        }

        let Some(next_char) = chars.get(index + 1) else {
            continue;
        };

        if !is_ident_start(*next_char) {
            continue;
        }

        let ident_end = ident_end(&chars, index + 1);
        if chars.get(ident_end) == Some(&'(') {
            panic!(
                "{label}:{line_number} should use OSV helper calls instead of object-style method calls"
            );
        }
    }
}

fn assert_no_traditional_function_call(label: &str, line_number: usize, line: &str) {
    let chars: Vec<_> = line.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if !is_ident_start(chars[index]) {
            index += 1;
            continue;
        }

        let ident_start = index;
        let ident_end = ident_end(&chars, ident_start);
        let ident: String = chars[ident_start..ident_end].iter().collect();

        if ident_start > 0 && chars[ident_start - 1] == '\\' {
            index = ident_end;
            continue;
        }

        if chars.get(ident_end) == Some(&'(') && !is_allowed_parenthesized_ident(&ident) {
            panic!(
                "{label}:{line_number} should use OSV calls instead of traditional function call `{ident}(...)`"
            );
        }

        index = ident_end;
    }
}

fn assert_no_future_form_syntax(label: &str, line_number: usize, line: &str) {
    let code = line.split("//").next().unwrap_or(line).trim();

    assert!(
        !contains_word(code, "form"),
        "{label}:{line_number} should not use future-only `form` declarations in release examples"
    );
    assert!(
        !contains_word(code, "takes"),
        "{label}:{line_number} should not use future-only `takes` adoption syntax in release examples"
    );
    assert!(
        !(contains_word(code, "where") && contains_word(code, "of")),
        "{label}:{line_number} should not use future-only `where T of Form` bounds in release examples"
    );
}

fn contains_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(index, _)| {
        let before = line[..index].chars().next_back();
        let after = line[index + word.len()..].chars().next();

        before.is_none_or(|char_| !is_ident_continue(char_))
            && after.is_none_or(|char_| !is_ident_continue(char_))
    })
}

fn ident_end(chars: &[char], start: usize) -> usize {
    let mut end = start + 1;

    while chars
        .get(end)
        .is_some_and(|char_| is_ident_continue(*char_))
    {
        end += 1;
    }

    end
}

fn is_allowed_parenthesized_ident(ident: &str) -> bool {
    ident.chars().next().is_some_and(char::is_uppercase)
}

fn is_ident_start(char_: char) -> bool {
    char_ == '_' || char_.is_ascii_alphabetic()
}

fn is_ident_continue(char_: char) -> bool {
    char_ == '_' || char_.is_ascii_alphanumeric()
}
