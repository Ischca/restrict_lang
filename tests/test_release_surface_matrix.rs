use std::fs;
use std::path::Path;

use restrict_lang::diagnostics::format_parse_error;
use restrict_lang::parser::parse_program;

const LANGUAGE_SPEC: &str = "LANGUAGE_SPECIFICATION.md";
const RELEASE_SURFACE_DOC: &str = "docs/v001-release-surface.md";

const REQUIRED_PHRASES: &[&str] = &[
    "## Supported",
    "## Rejected With Explicit Diagnostics",
    "## Experimental/Post-v0.0.1",
    "OSV-only calls",
    "`val` / `mut val`",
    "`List<T>`, `Option<T>`, `Result<T, E>`, and concrete `Range<Int32>`",
    "Fixed-length arrays",
    "not a source-level `Array<T, 0>` release contract",
    "Internal `Container` forms only for `List` / `Option`",
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
    "User enum/ADT reserved unsupported",
    "Exported generic/composite host ABI as design gap",
    "before `--check` success or code generation",
    "Source-level record exports no host ABI",
    "string import paths and import aliases are unsupported in v0.0.1",
    "re-exports are unsupported in v0.0.1",
    "enum declarations are unsupported in v0.0.1",
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
        label: "source-level form/takes",
        release_phrases: &[
            "Source-level `form` / `takes`",
            "Reserved for a later type-system pass",
        ],
        spec_phrases: &[
            "Source-level declarations spelled `form` / `takes`",
            "reserved design-space",
            "terminology for later type-system work",
            "outside the default v0.0.1",
        ],
    },
    OutsideGateSurface {
        label: "user-defined enum/ADT",
        release_phrases: &[
            "User enum/ADT reserved unsupported",
            "User-defined ADTs",
            "user-defined enum/ADT declarations remain unsupported",
        ],
        spec_phrases: &[
            "User-defined `enum`/ADT declarations",
            "outside default v0.0.1",
            "only compiler-provided `Option<T>` and `Result<T, E>` sum types are current",
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
fn v001_release_surface_and_spec_keep_unsupported_forms_outside_default_gate() {
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
fn v001_release_surface_supported_section_does_not_promote_reserved_work() {
    let doc = read_fixture(RELEASE_SURFACE_DOC);
    let supported = section_between(
        &doc,
        "## Supported",
        "## Rejected With Explicit Diagnostics",
    );
    for forbidden in [
        "TAT",
        "`form`",
        "`takes`",
        "source-level form",
        "generic export abi",
        "exported generic",
    ] {
        assert!(
            !supported.contains(forbidden),
            "{RELEASE_SURFACE_DOC} should not claim `{forbidden}` is supported"
        );
    }
}

#[test]
fn parser_rejects_source_level_form_takes_with_v001_container_boundary() {
    let cases = [
        (
            "form",
            r#"form Container<T> {
    Item
}
"#,
        ),
        (
            "takes",
            r#"takes List<T> Container {
    Item = T
}
"#,
        ),
    ];

    for (label, source) in cases {
        let err = parse_program(source).expect_err("source-level form/takes should not parse");
        let message = format_parse_error(source, err);

        assert!(
            message.contains("source-level `form` / `takes` syntax is unsupported in v0.0.1"),
            "{label} diagnostic should explain the v0.0.1 form/takes boundary, got: {message}"
        );
        assert!(
            message.contains("compiler-internal Container behavior"),
            "{label} diagnostic should identify the Container-only internal behavior, got: {message}"
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
        "{path} should mark {label} outside default v0.0.1 with phrase: {phrase}"
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
