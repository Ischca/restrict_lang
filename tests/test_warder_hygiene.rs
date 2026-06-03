use std::fs;
use std::path::Path;

#[test]
fn warder_new_templates_use_current_v001_syntax() {
    let source = read_workspace_file("warder/src/commands/new.rs");

    assert!(
        source.contains("fun main: () -> () ="),
        "`warder new` should generate the current main function syntax"
    );
    assert!(
        source.contains("fun test_example: () -> Boolean ="),
        "`warder new` should generate a type-checkable smoke test"
    );

    let stale_fragments = [
        "fun main =",
        "fun test_example =",
        "import std.test",
        "assert_eq(",
        "|>>",
    ];
    for fragment in stale_fragments {
        assert!(
            !source.contains(fragment),
            "`warder new` template should not contain stale Restrict syntax `{fragment}`"
        );
    }
}

#[test]
fn warder_init_template_uses_current_v001_syntax() {
    let source = read_workspace_file("warder/src/commands/init.rs");

    assert!(
        source.contains("fun main: () -> () ="),
        "`warder init` should generate the current main function syntax"
    );

    let stale_fragments = ["fun main =", "|>>"];
    for fragment in stale_fragments {
        assert!(
            !source.contains(fragment),
            "`warder init` template should not contain stale Restrict syntax `{fragment}`"
        );
    }
}

#[test]
fn checked_in_warder_fixture_sources_use_current_v001_syntax() {
    let fixture_paths = [
        "warder/test-warder/my-app/src/main.rl",
        "warder/test-warder/my-app/tests/main_test.rl",
        "warder/test-warder/my-app/test-warder-local/my-project/src/main.rl",
        "warder/test-warder/my-app/test-warder-local/my-project/tests/main_test.rl",
    ];

    for path in fixture_paths {
        let source = read_workspace_file(path);
        let stale_fragments = [
            "fun main =",
            "fun test_example =",
            "import std.test",
            "assert_eq(",
            "|>>",
        ];

        for fragment in stale_fragments {
            assert!(
                !source.contains(fragment),
                "{path} should not contain stale Restrict syntax `{fragment}`"
            );
        }
    }
}

#[test]
fn warder_test_uses_current_cli_check_mode() {
    let source = read_workspace_file("warder/src/commands/test.rs");

    assert!(
        source.contains(".arg(\"--check\")"),
        "`warder test` should use the current compiler type-check mode"
    );
    assert!(
        !source.contains(".arg(\"--test\")"),
        "`warder test` should not call the unsupported compiler --test flag"
    );
}

#[test]
fn warder_build_unimplemented_modes_are_release_scoped() {
    let source = read_workspace_file("warder/src/commands/build.rs");

    for anchor in [
        "Watch mode",
        "Release optimizations",
        "WASM Component output",
        "Deterministic build mode",
        "Signature verification",
    ] {
        assert_release_readiness_message(&source, anchor);
    }
}

#[test]
fn warder_publish_does_not_imply_registry_upload() {
    let source = read_workspace_file("warder/src/commands/publish.rs");

    assert_release_readiness_message(&source, "Registry publishing");
    assert!(
        source.contains("no package was uploaded"),
        "`warder publish` should explicitly say that no registry upload occurs"
    );
    assert!(
        !source.contains("Publishing not implemented yet"),
        "`warder publish` should avoid vague unimplemented messaging"
    );
    assert!(
        !source.contains("Publishing {} v{} to {}"),
        "`warder publish` should not print a success-looking upload message"
    );
}

#[test]
fn warder_wrap_experimental_paths_are_release_scoped() {
    let source = read_workspace_file("warder/src/commands/wrap.rs");

    assert_release_readiness_message(&source, "Foreign WASM wrapping");
    assert_release_readiness_message(&source, "WASM Component conversion");
    assert!(
        source.contains("Wrote experimental cage"),
        "`warder wrap` should label generated cages as experimental"
    );
    assert!(
        !source.contains("Wrapped WASM into cage"),
        "`warder wrap` should avoid success-looking output for an experimental path"
    );
    assert!(
        !source.contains("Component conversion not implemented yet"),
        "`warder unwrap --component` should avoid vague unimplemented messaging"
    );
}

#[test]
fn warder_doctor_skipped_checks_are_release_scoped() {
    let source = read_workspace_file("warder/src/commands/doctor.rs");

    assert_release_readiness_message(&source, "Public API freeze analysis");
    assert_release_readiness_message(&source, "Circular dependency analysis");
}

#[test]
fn warder_cage_abi_hash_uses_v001_content_framing() {
    let source = read_workspace_file("warder/src/cage.rs");

    assert!(
        source.contains("ABI_HASH_FORMAT_VERSION"),
        "cage ABI hashing should use an explicit versioned format"
    );
    assert!(
        source.contains("warder.cage.abi-content.v0.0.1"),
        "cage ABI hashing should stay scoped to the v0.0.1 content format"
    );
    assert!(
        source.contains("sort_unstable"),
        "WIT files should be canonicalized before hashing"
    );
    assert!(
        source.contains("module.wasm")
            && source.contains("wit.filename")
            && source.contains("wit.content"),
        "ABI/content hash should include WASM bytes and WIT filenames/content"
    );
    assert!(
        source.contains("to_le_bytes"),
        "ABI/content hash should use length framing to avoid concatenation ambiguity"
    );
    assert!(
        !source.contains("TODO: Implement proper ABI hash calculation"),
        "ABI hash calculation should not be left as a placeholder"
    );
    assert!(
        !source.contains("For now, use a simple hash of WASM + WIT content"),
        "ABI hash calculation should not describe itself as a temporary placeholder"
    );
}

fn assert_release_readiness_message(source: &str, anchor: &str) {
    let start = source
        .find(anchor)
        .unwrap_or_else(|| panic!("expected release-readiness message containing `{anchor}`"));
    let snippet = source[start..].chars().take(320).collect::<String>();

    for marker in ["v0.0.1", "out-of-scope", "experimental"] {
        assert!(
            snippet.contains(marker),
            "message near `{anchor}` should mention `{marker}`; snippet was:\n{snippet}"
        );
    }
}

fn read_workspace_file(relative_path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path))
        .unwrap_or_else(|err| panic!("{relative_path} should be readable: {err}"))
}
