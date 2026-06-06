// Range Literals
//
// Ranges use `..` for inclusive Int32 endpoints in v0.0.1.
//
// Syntax:
//   [start..end]   inclusive (both ends)

fun main: () -> Range<Int32> = {
    // Inclusive range: 1, 2, 3, 4, 5
    [1..5]
}
