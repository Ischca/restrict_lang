// v0.0.1 pure-computation smoke example.
// Demonstrates small total functions without runtime test DSL support.

fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun increment_twice: (value: Int32) -> Int32 = {
    val once = value |> increment;
    once |> increment
}

fun all_three: (first: Boolean, second: Boolean, third: Boolean) -> Boolean = {
    first && second && third
}

fun main: () -> Boolean = {
    val shifted = 5 |> increment_twice;
    val scaled = shifted |> double;

    (scaled == 14, shifted > 0, true) all_three
}
