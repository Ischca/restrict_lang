// Range Literals
//
// Ranges use Kotlin-style `..` for inclusive
// and Swift-style `..<` for exclusive.
//
// Syntax:
//   [start..end]   inclusive (both ends)
//   [start..<end]  exclusive (end not included)

fun main = {
    // Inclusive range: 1, 2, 3, 4, 5
    val r1 = [1..5]

    // Exclusive range: 1, 2, 3, 4
    val r2 = [1..<5]

    // Range in expression context
    val countdown = [10..1]

    0
}
