use std::fs;
use std::path::Path;

use restrict_lang::diagnostics::format_parse_error;
use restrict_lang::parser::parse_program;

const LANGUAGE_SPEC: &str = "LANGUAGE_SPECIFICATION.md";
const RELEASE_SURFACE_DOC: &str = "docs/v001-release-surface.md";

const REQUIRED_PHRASES: &[&str] = &[
    "## Supported",
    "## Rejected With Explicit Diagnostics",
    "## Experimental/Future",
    "OSV-only calls",
    "`val` / `mut val`",
    "`List<T>`, `Option<T>`, `Result<T, E>`, and concrete `Range<Int32>`",
    "Fixed-length arrays",
    "not a source-level `Array<T, 0>` release contract",
    "Built-in container behavior",
    "Source imports without aliases/re-exports",
    "Scalar monomorphic `pub fun` / `export fun` host ABI",
    "`Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()`",
    "immutable top-level literal constants",
    "Computed or mutable exported globals",
    "Exported top-level bindings must be immutable scalar constants in v0.0.1",
    "exported top-level bindings must be scalar literal constants in v0.0.1",
    "Program entry `main` emitted as `_start`",
    "main` is the source entry point",
    "TAT outside default gate",
    "Closed user-defined enums",
    "non-generic, non-recursive enums",
    "qualified `Type::Variant` names",
    "`pub enum` crosses source-module boundaries only",
    "no host-visible enum ABI",
    "Generic/recursive enums and `?`",
    "ergonomic `?` propagation is not implemented",
    "Method-only forms",
    "Concrete record `takes`",
    "Generic `of` bounds",
    "Standard `Display`",
    "Dispatch is static and monomorphized to direct method calls",
    "Associated types, generic forms, default methods",
    "Exported generic/composite host ABI as design gap",
    "before `--check` success or code generation",
    "Source-level record exports no host ABI",
    "string import paths and import aliases are unsupported in v0.0.1",
    "re-exports are unsupported in v0.0.1",
    "traditional calls like `add(1, 2)` are not valid Restrict",
];

struct OutsideGateSurface {
    label: &'static str,
    release_phrases: &'static [&'static str],
    spec_phrases: &'static [&'static str],
}

const OUTSIDE_GATE_SURFACES: &[OutsideGateSurface] = &[
    OutsideGateSurface {
        label: "TAT",
        release_phrases: &[
            "TAT outside default gate",
            "outside the v0.0.1 default release gate",
        ],
        spec_phrases: &[
            "Temporal Affine Types (TAT)",
            "outside the default v0.0.1 gate",
            "Temporal Resource Management (Experimental / Outside v0.0.1 Default Gate)",
        ],
    },
    OutsideGateSurface {
        label: "exported generic/composite host ABI",
        release_phrases: &[
            "Exported generic/composite host ABI as design gap",
            "Host-visible WebAssembly ABI rules for exported generic and composite values",
            "not a supported release contract",
        ],
        spec_phrases: &[
            "generic/composite host ABI",
            "including exported generic functions or direct",
            "exported record values",
            "remain outside default v0.0.1",
        ],
    },
];

#[test]
fn v001_release_surface_matrix_preserves_key_contracts() {
    let doc = read_fixture(RELEASE_SURFACE_DOC);

    for phrase in REQUIRED_PHRASES {
        assert!(
            doc.contains(phrase),
            "{RELEASE_SURFACE_DOC} should preserve required phrase: {phrase}"
        );
    }
}

#[test]
fn parser_rejects_traditional_calls_with_v001_osv_boundary() {
    let source = r#"
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    add(1, 2)
}
"#;
    let err = parse_program(source).expect_err("traditional call syntax should not parse");
    let message = format_parse_error(source, err);

    assert!(
        message.contains(
            "traditional calls like `add(1, 2)` are not valid Restrict; use OSV syntax such as `(1, 2) add` or `value |> add`"
        ),
        "traditional call diagnostic should explain the OSV-only boundary, got: {message}"
    );
    for internal in [
        "unexpected input near",
        "Error(",
        "ErrorKind",
        "nom::",
        "Tag",
        "Alt",
    ] {
        assert!(
            !message.contains(internal),
            "traditional call diagnostic should not expose parser internals ({internal}), got: {message}"
        );
    }
}

#[test]
fn v001_release_surface_and_spec_keep_deferred_surfaces_outside_default_gate() {
    let release_surface = read_fixture(RELEASE_SURFACE_DOC);
    let language_spec = read_fixture(LANGUAGE_SPEC);

    for surface in OUTSIDE_GATE_SURFACES {
        for phrase in surface.release_phrases {
            assert_doc_contains(RELEASE_SURFACE_DOC, &release_surface, phrase, surface.label);
        }
        for phrase in surface.spec_phrases {
            assert_doc_contains(LANGUAGE_SPEC, &language_spec, phrase, surface.label);
        }
    }
}

#[test]
fn release_surface_and_spec_define_the_v001_static_form_boundary() {
    let release_surface = read_fixture(RELEASE_SURFACE_DOC);
    let language_spec = read_fixture(LANGUAGE_SPEC);
    let normalized_language_spec = normalize_whitespace(&language_spec);
    for phrase in [
        "Method-only forms",
        "fully typed method signatures",
        "Concrete record `takes`",
        "concrete, non-generic record",
        "Generic `of` bounds",
        "<T of First + Second>",
        "static and monomorphized to direct method calls",
        "Standard `Display`",
        "stderr remains String-only",
        "Advanced form features",
        "Associated types, generic forms, default methods",
    ] {
        assert_doc_contains(
            RELEASE_SURFACE_DOC,
            &release_surface,
            phrase,
            "v0.0.1 static forms",
        );
    }

    for phrase in [
        "Forms, Adoptions, and Form Bounds",
        "surface is intentionally method-only",
        "A `takes` declaration targets one concrete, non-generic record",
        "<T of A + B>",
        "monomorphizes form-bounded generic calls and emits a direct call",
        "Passing a non-Copy value as `self` consumes it",
        "does not include associated types, generic forms",
        "pub form Display",
        "compiler-provided `Display` adoptions",
        "eprint` and `eprintln` remain String-only",
    ] {
        assert_doc_contains(
            LANGUAGE_SPEC,
            &normalized_language_spec,
            phrase,
            "v0.0.1 static forms",
        );
    }
}

#[test]
fn release_surface_and_spec_define_the_v001_closed_enum_boundary() {
    let release_surface = read_fixture(RELEASE_SURFACE_DOC);
    let language_spec = read_fixture(LANGUAGE_SPEC);
    let normalized_language_spec = normalize_whitespace(&language_spec);
    for phrase in [
        "Closed user-defined enums",
        "non-generic, non-recursive enums",
        "zero or one payload",
        "qualified `Type::Variant` names",
        "matches are exhaustive",
        "`pub enum` crosses source-module boundaries only",
        "no host-visible enum ABI",
        "Generic/recursive enums and `?`",
        "`Result<T, CustomError>`",
        "ergonomic `?` propagation is not implemented",
    ] {
        assert_doc_contains(
            RELEASE_SURFACE_DOC,
            &release_surface,
            phrase,
            "v0.0.1 closed user-defined enums",
        );
    }

    for phrase in [
        "closed, non-generic user-defined `enum` slice",
        "Generic enums, recursive enums",
        "variants are scoped under their enum name",
        "pub enum PublicError",
        "Exported enums have the same source-module-only meaning",
        "do not emit direct host-visible WebAssembly exports",
        "user-defined enums are rejected by both the default ABI and `flat-record-v1`",
        "A postfix `?` operator remains future work",
    ] {
        assert_doc_contains(
            LANGUAGE_SPEC,
            &normalized_language_spec,
            phrase,
            "v0.0.1 closed user-defined enums",
        );
    }
}

#[test]
fn v001_release_surface_drops_superseded_enum_and_form_history() {
    let release_surface = read_fixture(RELEASE_SURFACE_DOC);

    for stale_claim in [
        "post-v0.0.1",
        "Post-v0.0.1",
        "Added after v0.0.1",
        "Historically rejected",
        "Historically reserved",
    ] {
        assert!(
            !release_surface.contains(stale_claim),
            "the first v0.0.1 release contract should not keep superseded history: {stale_claim}"
        );
    }
}

#[test]
fn flat_record_v1_spec_requires_reinstantiation_after_an_escaping_trap() {
    let language_spec = read_fixture(LANGUAGE_SPEC);
    for phrase in [
        "must treat that module instance as invalid",
        "instantiate a fresh module",
    ] {
        assert!(
            language_spec.contains(phrase),
            "{LANGUAGE_SPEC} should define the escaping-trap instance boundary: {phrase}"
        );
    }
}

#[test]
fn v001_release_surface_supported_section_does_not_promote_reserved_work() {
    let doc = read_fixture(RELEASE_SURFACE_DOC);
    let supported = section_between(
        &doc,
        "## Supported",
        "## Rejected With Explicit Diagnostics",
    );
    for forbidden in ["TAT", "generic export abi", "exported generic"] {
        assert!(
            !supported.contains(forbidden),
            "{RELEASE_SURFACE_DOC} should not claim `{forbidden}` is supported"
        );
    }

    for required in ["Source-level forms", "`takes`", "Closed user-defined enums"] {
        assert!(
            supported.contains(required),
            "{RELEASE_SURFACE_DOC} should include v0.0.1 surface `{required}`"
        );
    }
}

#[test]
fn parser_accepts_current_form_takes_and_of_surface() {
    let source = r#"
pub form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value |> label
}
"#;
    let (remaining, _) = parse_program(source).expect("current form syntax should parse");
    assert!(
        remaining.trim().is_empty(),
        "current form syntax should parse completely, remaining: {remaining:?}"
    );
}

#[test]
fn parser_rejects_deferred_form_features_with_specific_diagnostics() {
    let cases = [
        (
            "generic form",
            r#"form Labelled<T> {
    fun label: (self: Self) -> String
}
"#,
            "generic forms are not supported yet",
        ),
        (
            "associated type",
            r#"form Container {
    type Item
}
"#,
            "source-level associated form types are not supported yet",
        ),
    ];

    for (label, source, expected) in cases {
        let err = parse_program(source).expect_err("deferred form syntax should not parse");
        let message = format_parse_error(source, err);

        assert!(
            message.contains(expected),
            "{label} diagnostic should explain the deferred form feature, got: {message}"
        );
        for internal in [
            "unexpected input near",
            "Error(",
            "ErrorKind",
            "nom::",
            "Tag",
            "Alt",
        ] {
            assert!(
                !message.contains(internal),
                "{label} diagnostic should not expose parser internals ({internal}), got: {message}"
            );
        }
    }
}

fn read_fixture(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn assert_doc_contains(path: &str, doc: &str, phrase: &str, label: &str) {
    assert!(
        doc.contains(phrase),
        "{path} should document {label} with phrase: {phrase}"
    );
}

fn section_between<'a>(doc: &'a str, start: &str, end: &str) -> &'a str {
    let section_start = doc
        .find(start)
        .unwrap_or_else(|| panic!("missing section start marker: {start}"));
    let content_start = section_start + start.len();
    let section_end = doc[content_start..]
        .find(end)
        .map(|offset| content_start + offset)
        .unwrap_or_else(|| panic!("missing section end marker: {end}"));

    &doc[content_start..section_end]
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
