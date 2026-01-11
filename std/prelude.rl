// Restrict Language Standard Prelude
// 標準ライブラリ: Prelude（自動インポート）
//
// This module is automatically imported into every Restrict program.
// All exported functions are available without explicit import.

// ============================================================
// I/O Functions (built-in, re-exported for documentation)
// ============================================================

// print<T: Display> - Print any displayable value
// println<T: Display> - Print with newline
// print_int - Print integer
// print_float - Print float
// These are built-in and handled by the type checker/codegen

// ============================================================
// Boolean Operations
// ============================================================

// Logical NOT
export fun not: (b: Bool) -> Bool = {
    b match {
        true => { false }
        false => { true }
    }
}

// Note: and, or, xor require using variables in multiple match arms
// which conflicts with affine types. Use && and || operators instead.

// ============================================================
// Identity Functions
// ============================================================

// Identity function for Int
export fun identity_int: (x: Int) -> Int = {
    x
}

// Identity function for Bool
export fun identity_bool: (x: Bool) -> Bool = {
    x
}

// ============================================================
// Comparison Helpers
// ============================================================

// Check if two integers are equal
export fun eq_int: (a: Int, b: Int) -> Bool = {
    a == b
}

// Check if two integers are not equal
export fun ne_int: (a: Int, b: Int) -> Bool = {
    a != b
}

// Check if a < b
export fun lt_int: (a: Int, b: Int) -> Bool = {
    a < b
}

// Check if a <= b
export fun le_int: (a: Int, b: Int) -> Bool = {
    a <= b
}

// Check if a > b
export fun gt_int: (a: Int, b: Int) -> Bool = {
    a > b
}

// Check if a >= b
export fun ge_int: (a: Int, b: Int) -> Bool = {
    a >= b
}

// ============================================================
// Arithmetic Helpers
// ============================================================

// Add two integers
export fun add: (a: Int, b: Int) -> Int = {
    a + b
}

// Subtract: a - b
export fun sub: (a: Int, b: Int) -> Int = {
    a - b
}

// Multiply
export fun mul: (a: Int, b: Int) -> Int = {
    a * b
}

// Divide (integer division)
export fun div: (a: Int, b: Int) -> Int = {
    a / b
}

// Modulo
export fun mod: (a: Int, b: Int) -> Int = {
    a % b
}

// Negate
export fun neg: (x: Int) -> Int = {
    0 - x
}

// ============================================================
// Option Constructors (if not built-in)
// ============================================================

// These are typically built-in but documented here for reference:
// Some(x) - Wrap a value in Some
// None - The absence of a value

// ============================================================
// Unit value
// ============================================================

// Return unit (useful for side-effect functions)
export fun unit: () -> Unit = {
    ()
}
