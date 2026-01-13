// Restrict Language Standard Library: String Operations
// 標準ライブラリ: 文字列操作
//
// Core string functions are implemented as WASM built-ins and always available.
// This module provides additional string utilities.

// ============================================================
// WASM Built-in Functions (always available)
// ============================================================
//
// string_length: (String) -> Int
//     Get the length of a string
//
// string_concat: (String, String) -> String
//     Concatenate two strings
//
// string_equals: (String, String) -> Bool
//     Compare two strings for equality
//
// char_at: (String, Int) -> Int
//     Get character code at index (returns -1 if out of bounds)
//
// substring: (String, Int, Int) -> String
//     Extract portion of string (start inclusive, end exclusive)
//
// string_to_int: (String) -> Int
//     Parse integer from string (returns 0 on invalid input)
//
// int_to_string: (Int) -> String
//     Format integer as string

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

// Convert a boolean to a string
export fun bool_to_string: (b: Bool) -> String = {
    b then { "true" } else { "false" }
}

// Get the first character of a string (or -1 if empty)
export fun string_head: (s: String) -> Int = {
    (s, 0) char_at
}

// Get first n characters
export fun string_take: (s: String, n: Int) -> String = {
    (s, 0, n) substring
}

// Note: string_tail and string_drop require using the string twice
// (once for length, once for substring), which violates affine types.
// Use substring directly with explicit indices instead.

// ============================================================
// Character Operations
// ============================================================

// Check if a character is a digit (0-9)
export fun is_digit: (c: Char) -> Bool = {
    c >= '0' && c <= '9'
}

// Check if a character is a lowercase letter (a-z)
export fun is_lower: (c: Char) -> Bool = {
    c >= 'a' && c <= 'z'
}

// Check if a character is an uppercase letter (A-Z)
export fun is_upper: (c: Char) -> Bool = {
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
    c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

// Convert a character to its ASCII code
export fun char_to_int: (c: Char) -> Int = {
    c
}

// Convert an ASCII code to a character
export fun int_to_char: (n: Int) -> Char = {
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
        ((c) char_to_int - 32) int_to_char
    } else {
        c
    }
}

// Convert uppercase to lowercase
export fun to_lower: (c: Char) -> Char = {
    (c) is_upper then {
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
