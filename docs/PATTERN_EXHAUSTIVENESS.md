# Pattern Matching Exhaustiveness Checking

This document describes the comprehensive pattern matching exhaustiveness checking system implemented in the Restrict Language compiler.

## Overview

Exhaustiveness checking ensures that pattern matching expressions cover all possible cases of the matched type, preventing runtime errors and forcing developers to handle all scenarios explicitly.

## Features

### 1. Comprehensive Type Coverage

The exhaustiveness checker analyzes different types:

- **Boolean types**: Requires both `true` and `false` cases
- **Option types**: Requires both `Some(_)` and `None` cases
- **Unit type**: Requires `()` case
- **List types**: Requires `[]` and non-empty cases
- **Record types**: Requires at least one record pattern
- **Infinite types** (Int32, String, etc.): Requires wildcard pattern

### 2. Nested Pattern Analysis

The checker recursively analyzes nested patterns:

```restrict
// This is exhaustive
option_bool |> match {
    Some(true) => 1,
    Some(false) => 0,
    None => -1
}

// This is NOT exhaustive - missing Some(false)
option_bool |> match {
    Some(true) => 1,
    None => -1
}
```

### 3. Helpful Error Messages

When patterns are non-exhaustive, the compiler provides specific missing patterns:

```
Error: Non-exhaustive patterns: missing Some(false).
Add the missing patterns or use a wildcard pattern (_).
```

### 4. Wildcard Escape Hatch

Wildcard patterns (`_`) and identifier patterns make any match exhaustive:

```restrict
value |> match {
    42 => "special",
    _ => "other"  // Covers all remaining cases
}
```

## Type-Specific Behavior

### Boolean Types

```restrict
// ✓ Exhaustive
flag |> match {
    true => "yes",
    false => "no"
}

// ✗ Non-exhaustive: missing false
flag |> match {
    true => "yes"
}
```

### Option Types

```restrict
// ✓ Exhaustive
maybe |> match {
    Some(value) => value,
    None => 0
}

// ✗ Non-exhaustive: missing None
maybe |> match {
    Some(value) => value
}
```

### List Types

```restrict
// ✓ Exhaustive
list |> match {
    [] => "empty",
    [head | tail] => "non-empty"
}

// ✗ Non-exhaustive: missing []
list |> match {
    [head | tail] => "non-empty"
}

// ✗ Non-exhaustive: exact patterns need cons pattern for completeness
list |> match {
    [] => "empty",
    [a] => "one",
    [a, b] => "two"
    // Missing longer lists - need [head | tail] or wildcard
}
```

### Record Types

```restrict
record Point { x: Int32, y: Int32 }

// ✓ Exhaustive
point |> match {
    Point { x, y } => x + y
}

// ✗ Non-exhaustive: missing Point pattern
point |> match {
    // No patterns at all
}
```

### Infinite Types

For types with infinite possible values (Int32, String, Float64, Char), exhaustiveness checking requires a wildcard:

```restrict
// ✓ Exhaustive
number |> match {
    0 => "zero",
    1 => "one",
    _ => "other"
}

// ✗ Non-exhaustive: pattern required for infinite type
number |> match {
    0 => "zero",
    1 => "one"
    // Missing wildcard for infinite Int32 space
}
```

## Advanced Features

### Nested Pattern Exhaustiveness

The checker recursively analyzes nested structures:

```restrict
// ✓ Exhaustive - all combinations covered
nested |> match {
    Some(Some(value)) => value,
    Some(None) => 0,
    None => -1
}

// ✗ Non-exhaustive: missing Some(None)
nested |> match {
    Some(Some(value)) => value,
    None => -1
}
```

### List Head Pattern Analysis

For list cons patterns, the checker analyzes head pattern completeness:

```restrict
// If matching Option<Int32> list elements:

// ✓ Exhaustive
list |> match {
    [] => 0,
    [Some(x) | tail] => x,
    [None | tail] => 0
}

// ✗ Non-exhaustive: missing [None|_]
list |> match {
    [] => 0,
    [Some(x) | tail] => x
}
```

## Implementation Details

### Pattern Space Analysis

The exhaustiveness checker uses pattern space analysis:

1. **Pattern Matrix Construction**: Builds a matrix of patterns from all match arms
2. **Coverage Analysis**: Determines what portion of the type space is covered
3. **Gap Detection**: Identifies uncovered patterns
4. **Error Generation**: Produces helpful error messages for missing patterns

### Performance Considerations

- **Lazy Evaluation**: Only analyzes patterns when exhaustiveness is questioned
- **Early Termination**: Stops analysis when wildcard patterns are found
- **Efficient Algorithms**: Uses set-based operations for coverage analysis
- **Bounded Recursion**: Prevents infinite recursion in self-referential types

## Benefits

1. **Prevents Runtime Errors**: Catches missing cases at compile time
2. **Improves Code Quality**: Forces explicit handling of all scenarios
3. **Self-Documenting**: Makes code intentions clear through complete pattern coverage
4. **Refactoring Safety**: Adding new enum variants causes compile errors until all matches are updated
5. **Debugging Aid**: Clear error messages guide developers to missing cases

## Comparison with Other Languages

This implementation is inspired by exhaustiveness checking in:

- **Rust**: Similar pattern analysis but adapted for Restrict's affine types
- **ML/OCaml**: Classical exhaustiveness checking algorithms
- **Haskell**: Pattern match completeness warnings
- **Elm**: Comprehensive pattern coverage analysis

## Future Enhancements

Potential improvements:

1. **Guard Conditions**: Exhaustiveness analysis with pattern guards
2. **Numeric Range Analysis**: Smarter checking for numeric literal patterns
3. **Custom Types**: Exhaustiveness for user-defined sum types
4. **Performance Optimization**: Faster algorithms for complex pattern matrices
5. **IDE Integration**: Real-time exhaustiveness feedback in editors

## Error Reference

### NonExhaustivePatterns

```
Non-exhaustive patterns: missing {patterns}. {suggestion}
```

**Cause**: Match expression doesn't cover all possible cases of the matched type.

**Solution**:
- Add the missing patterns listed in the error
- Use a wildcard pattern (`_`) to cover remaining cases
- Use an identifier pattern to bind and handle remaining cases

### Examples of Error Messages

```
Non-exhaustive patterns: missing false. Add the missing patterns or use a wildcard pattern (_).

Non-exhaustive patterns: missing None. Add the missing patterns or use a wildcard pattern (_).

Non-exhaustive patterns: missing []. Add the missing patterns or use a wildcard pattern (_).

Non-exhaustive patterns: missing Some(false). Add the missing patterns or use a wildcard pattern (_).

Non-exhaustive patterns: missing Point{ .. }. Add the missing patterns or use a wildcard pattern (_).
```

This exhaustiveness checking system makes Restrict Language programs more reliable and maintainable by catching incomplete pattern matches at compile time.