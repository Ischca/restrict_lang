use std::fs;
use std::path::Path;

const RUSTDOC_PATHS: &[&str] = &["src/ast.rs", "src/lib.rs", "src/parser.rs"];

#[test]
fn rustdoc_restrict_examples_do_not_use_removed_surface_syntax() {
    for path in RUSTDOC_PATHS {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("{path} should be readable: {err}"));

        for (lang, block) in rustdoc_code_blocks(&source) {
            if !is_restrict_surface_block(path, &lang) {
                continue;
            }

            assert_no_removed_syntax(path, &block);
        }
    }
}

fn rustdoc_code_blocks(source: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current_lang: Option<String> = None;
    let mut current_block = String::new();

    for line in source.lines() {
        let Some(doc_line) = strip_doc_prefix(line) else {
            continue;
        };
        let trimmed = doc_line.trim_start();

        if let Some(fence_lang) = trimmed.strip_prefix("```") {
            if let Some(lang) = current_lang.take() {
                blocks.push((lang, current_block.clone()));
                current_block.clear();
            } else {
                current_lang = Some(fence_lang.trim().to_string());
            }
            continue;
        }

        if current_lang.is_some() {
            current_block.push_str(doc_line);
            current_block.push('\n');
        }
    }

    blocks
}

fn strip_doc_prefix(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let stripped = trimmed
        .strip_prefix("///")
        .or_else(|| trimmed.strip_prefix("//!"))?;
    Some(stripped.strip_prefix(' ').unwrap_or(stripped))
}

fn is_restrict_surface_block(path: &str, lang: &str) -> bool {
    lang == "restrict" || (Path::new(path) == Path::new("src/lib.rs") && lang == "text")
}

fn assert_no_removed_syntax(path: &str, block: &str) {
    let removed_patterns = [
        ("fn ", "use `fun name: (...) = { ... }`"),
        ("let ", "use `val`"),
        ("++", "use `+` for string concatenation"),
        ("add(", "use OSV `(args) add`"),
        ("map(", "use OSV `(args) map`"),
        ("\nif ", "use `condition then { ... }`"),
        ("\nwhile ", "use `condition while { ... }`"),
        (".read(", "use OSV `value |> read`"),
        (
            "import \"",
            "use dotted source imports such as `import release.{item}`",
        ),
    ];

    for (pattern, replacement) in removed_patterns {
        assert!(
            !block.contains(pattern),
            "{path} rustdoc example uses removed syntax `{pattern}`; {replacement}\n\n{block}"
        );
    }

    for line in block.lines() {
        let trimmed = line.trim_start();
        assert!(
            !(trimmed.starts_with("import ") && trimmed.contains(" as ")),
            "{path} rustdoc example uses import aliases, which are outside the v0.0.1 module surface:\n\n{block}"
        );
    }
}
