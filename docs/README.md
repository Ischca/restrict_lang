# Restrict Language Documentation

This directory contains the documentation for Restrict Language, including both user-facing guides and API documentation.

## Structure

```
docs/
├── en/                 # English documentation
├── ja/                 # Japanese documentation
├── includes/           # Shared code snippets (using mdBook includes)
├── code-examples/      # Standalone code example files
├── theme/              # Custom CSS and JavaScript
├── scripts/            # Documentation build and maintenance scripts
├── book.toml           # mdBook configuration
└── SUMMARY.md          # Table of contents
```

## Shared Code Examples

To avoid duplication between English and Japanese documentation, we use several strategies:

### 1. mdBook Includes (Recommended)

Create shared snippets in `includes/` directory:

```markdown
<!-- In docs/includes/hello.md -->
```restrict
fn main() {
    "Hello, World!" |> println
}
```
```

Then include in both EN and JA docs:

```markdown
<!-- In docs/en/getting-started/hello-world.md -->
{{#include ../../includes/hello.md}}

<!-- In docs/ja/getting-started/hello-world.md -->
{{#include ../../includes/hello.md}}
```

### 2. Code Example Files

For complete runnable examples, store them in `code-examples/`:

```bash
docs/code-examples/
├── hello-world.rl
├── osv-demo.rl
└── warder-demo/
    ├── package.rl.toml
    └── src/main.rl
```

Reference them in documentation:

```markdown
The complete example can be found in `docs/code-examples/hello-world.rl`.
```

### 3. Test Integration

Link documentation examples with actual tests:

```rust
// In tests/doc_examples.rs
#[test]
fn test_hello_world_example() {
    let code = include_str!("../docs/code-examples/hello-world.rl");
    assert_compiles(code);
}
```

## Building Documentation

```bash
# Build all documentation
mise run doc-all

# Build and serve locally
mise run doc-book

# Check translations
mise run doc-check-translations

# Validate documentation
mise run doc-validate
```

## Translation Workflow

1. Edit English documentation first
2. Run `mise run doc-check-translations` to see what needs updating
3. Update Japanese translations, keeping code examples identical
4. Commit both versions together

## Style Guidelines

### Code Examples

- Keep examples concise and focused
- Use meaningful variable names
- Include comments only when necessary
- Show both correct usage and common errors

### Language

- **English**: Clear, concise technical writing
- **Japanese**: Professional technical Japanese (敬語不要)

### Formatting

- Use ATX-style headers (`#`, not underlines)
- Indent code blocks with 4 spaces
- Use backticks for inline code
- Add language identifiers to code blocks

## Adding New Documentation

1. Create the English version first
2. Add entry to `SUMMARY.md`
3. If including code, add to `includes/` or `code-examples/`
4. Create Japanese translation
5. Run validation: `mise run doc-validate`

## Common Patterns

### Feature Introduction

```markdown
## Feature Name

Brief description of what the feature does.

### Why It Matters

Explain the problem it solves.

### Basic Usage

{{#include ../includes/feature-basic.md}}

### Advanced Usage

{{#include ../includes/feature-advanced.md}}

### Common Pitfalls

- Pitfall 1: Description
- Pitfall 2: Description
```

### API Documentation

Generate from source code comments:

```rust
/// Brief description.
/// 
/// Longer explanation with examples.
/// 
/// # Examples
/// 
/// ```restrict
/// example code
/// ```
pub fn function_name() { }
```

## Contributing

When contributing documentation:

1. Follow the existing style
2. Test all code examples
3. Update both EN and JA versions
4. Run `mise run doc-validate` before submitting