// Restrict Language Standard Library: String Operations
// 標準ライブラリ: 文字列操作
//
// Note: Core string functions (string_length, string_concat, string_equals)
// are implemented as WASM built-ins and are always available.
// This module provides additional string utilities.

// ============================================================
// String Properties (WASM Built-ins)
// ============================================================
//
// The following functions are built-in and always available:
//
// string_length: (String) -> Int
//     Get the length of a string
//
// string_concat: (String, String) -> String
//     Concatenate two strings
//
// string_equals: (String, String) -> Bool
//     Compare two strings for equality

// ============================================================
// String Utilities
// ============================================================

// Check if a string is empty
export fun string_is_empty: (s: String) -> Bool = {
    (s) string_length == 0
}

// Check if two strings are not equal
export fun string_not_equals: (a: String, b: String) -> Bool = {
    (a, b) string_equals then { false } else { true }
}

// Append a string to another (alias for string_concat)
export fun string_append: (base: String, suffix: String) -> String = {
    (base, suffix) string_concat
}

// ============================================================
// String Conversion (placeholders - need WASM runtime)
// ============================================================

// Convert a string to an integer
// Returns 0 if parsing fails
// TODO: Implement parsing logic at WASM level
export fun string_to_int: (s: String) -> Int = {
    // Placeholder - needs WASM runtime support
    0
}

// Convert an integer to a string
// TODO: Implement digit conversion at WASM level
export fun int_to_string: (n: Int) -> String = {
    // Placeholder - needs WASM runtime support
    "0"
}

// Convert a boolean to a string
export fun bool_to_string: (b: Bool) -> String = {
    b then { "true" } else { "false" }
}

// ============================================================
// Character Operations
// ============================================================

// Check if a character is a digit (0-9)
export fun is_digit: (c: Char) -> Bool = {
    // Char is represented as i32 (ASCII/Unicode code point)
    // '0' = 48, '9' = 57
    c >= '0' && c <= '9'
}

// Check if a character is a lowercase letter (a-z)
export fun is_lower: (c: Char) -> Bool = {
    // 'a' = 97, 'z' = 122
    c >= 'a' && c <= 'z'
}

// Check if a character is an uppercase letter (A-Z)
export fun is_upper: (c: Char) -> Bool = {
    // 'A' = 65, 'Z' = 90
    c >= 'A' && c <= 'Z'
}

// Check if a character is a letter (a-z or A-Z)
export fun is_alpha: (c: Char) -> Bool = {
    (c) is_lower || (c) is_upper
}

// Check if a character is alphanumeric (letter or digit)
export fun is_alphanumeric: (c: Char) -> Bool = {
    (c) is_alpha || (c) is_digit
}

// Check if a character is whitespace (space, tab, newline, etc.)
export fun is_whitespace: (c: Char) -> Bool = {
    // ' ' = 32, '\t' = 9, '\n' = 10, '\r' = 13
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

// Convert a character to its ASCII code
export fun char_to_int: (c: Char) -> Int = {
    // Chars are stored as i32 code points
    c
}

// Convert an ASCII code to a character
export fun int_to_char: (n: Int) -> Char = {
    // Direct conversion from code point
    n
}

// Convert a digit character to its numeric value (0-9)
// Returns -1 if not a digit
export fun digit_value: (c: Char) -> Int = {
    (c) is_digit then { (c) char_to_int - 48 } else { 0 - 1 }
}

// Convert lowercase to uppercase
export fun to_upper: (c: Char) -> Char = {
    (c) is_lower then {
        // Subtract 32 to convert lowercase to uppercase in ASCII
        ((c) char_to_int - 32) int_to_char
    } else {
        c
    }
}

// Convert uppercase to lowercase
export fun to_lower: (c: Char) -> Char = {
    (c) is_upper then {
        // Add 32 to convert uppercase to lowercase in ASCII
        ((c) char_to_int + 32) int_to_char
    } else {
        c
    }
}

// ============================================================
// String Constants
// ============================================================

// Get an empty string
export fun empty_string: () -> String = {
    ""
}

// Get a newline string
export fun newline: () -> String = {
    "\n"
}

// Get a space string
export fun space: () -> String = {
    " "
}

// Get a tab string
export fun tab: () -> String = {
    "\t"
}
