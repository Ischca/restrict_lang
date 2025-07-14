# Language Guide Overview

Welcome to the Restrict Language guide! This section covers all aspects of the language in detail.

## What You'll Learn

- **[OSV Word Order](./osv-order.md)** - Understanding the Object-Subject-Verb syntax
- **[Syntax Reference](./syntax.md)** - Complete syntax guide
- **[Type System](./types.md)** - Affine types and type safety
- **[Ownership](./ownership.md)** - Memory management without GC
- **[Pattern Matching](./patterns.md)** - Exhaustive pattern matching
- **[Warder Package Manager](./warder.md)** - Managing dependencies

## Quick Example

```restrict
// OSV syntax in action
fn processData(data: Vec<String>) -> Result<String, Error> {
    data
        |> filter(|s| !s.isEmpty())
        |> map(|s| s |> toUpperCase)
        |> join(", ")
        |> Ok
}

fn main() {
    let items = vec!["hello", "", "world"];
    
    match items |> processData {
        Ok(result) => result |> println,
        Err(e) => e.toString() |> println
    }
}
```

## Key Concepts

### Affine Types
Each value can be used at most once, preventing memory bugs at compile time.

### OSV Syntax
Natural data flow using the pipe operator `|>`.

### Pattern Matching
Exhaustive pattern matching ensures all cases are handled.

### Zero GC
Memory safety without garbage collection through compile-time checks.

Let's dive in!