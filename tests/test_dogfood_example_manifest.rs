use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const INTENTIONAL_DOGFOOD_EXAMPLE_GAPS: &[(&str, &str)] = &[(
    "dogfood_generic_export_gap.rl",
    "intentional v0.0.1 generic export ABI gap; parse/type-check only until host ABI is designed",
)];

#[test]
fn dogfood_examples_have_dedicated_tests_or_explicit_gap_entries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let examples = discover_dogfood_examples(root);
    assert!(
        !examples.is_empty(),
        "expected at least one examples/dogfood_*.rl dogfood example"
    );

    let gap_allowlist = intentional_gap_allowlist();
    assert_allowlist_is_current(&examples, &gap_allowlist);

    let mut missing = Vec::new();
    for example in &examples {
        if gap_allowlist.contains_key(example.as_str()) {
            continue;
        }

        let expected_test = dedicated_test_file_name(example);
        let expected_test_path = root.join("tests").join(&expected_test);
        let test_source = match fs::read_to_string(&expected_test_path) {
            Ok(source) => source,
            Err(err) => {
                missing.push(format!(
                    "examples/{example} -> missing tests/{expected_test}: {err}"
                ));
                continue;
            }
        };

        let expected_reference = format!("../examples/{example}");
        if !test_source.contains(&expected_reference) {
            missing.push(format!(
                "examples/{example} -> tests/{expected_test} does not reference {expected_reference:?}"
            ));
        }

        if !has_runtime_execution_coverage(&test_source) {
            missing.push(format!(
                "examples/{example} -> tests/{expected_test} should execute generated Wasm \
                 with a host-callable wrapper, or move the example to an intentional-gap allowlist"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "Every examples/dogfood_*.rl file needs a corresponding \
         dedicated runtime test or an intentional-gap allowlist entry.\n\n{}",
        missing.join("\n")
    );
}

fn discover_dogfood_examples(root: &Path) -> BTreeSet<String> {
    let examples_dir = root.join("examples");
    let entries = fs::read_dir(&examples_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", examples_dir.display()));

    entries
        .map(|entry| {
            entry
                .unwrap_or_else(|err| {
                    panic!(
                        "failed to read an entry from {}: {err}",
                        examples_dir.display()
                    )
                })
                .file_name()
                .into_string()
                .expect("example filenames should be valid UTF-8")
        })
        .filter(|file_name| file_name.starts_with("dogfood_") && file_name.ends_with(".rl"))
        .collect()
}

fn intentional_gap_allowlist() -> BTreeMap<&'static str, &'static str> {
    let allowlist = INTENTIONAL_DOGFOOD_EXAMPLE_GAPS
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();

    let blank_reasons = allowlist
        .iter()
        .filter_map(|(example, reason)| reason.trim().is_empty().then_some(*example))
        .collect::<Vec<_>>();
    assert!(
        blank_reasons.is_empty(),
        "intentional dogfood gap entries need a non-empty reason: {}",
        blank_reasons.join(", ")
    );

    allowlist
}

fn assert_allowlist_is_current(
    examples: &BTreeSet<String>,
    allowlist: &BTreeMap<&'static str, &'static str>,
) {
    let stale_entries = allowlist
        .keys()
        .copied()
        .filter(|example| !examples.contains(*example))
        .collect::<Vec<_>>();

    assert!(
        stale_entries.is_empty(),
        "intentional dogfood gap allowlist contains entries that no longer match \
         examples/dogfood_*.rl: {}",
        stale_entries.join(", ")
    );
}

fn dedicated_test_file_name(example: &str) -> String {
    let stem = example
        .strip_suffix(".rl")
        .expect("dogfood examples should use .rl files");
    format!("test_{stem}.rs")
}

fn has_runtime_execution_coverage(test_source: &str) -> bool {
    test_source.contains("_executes") && test_source.contains("get_typed_func")
}
