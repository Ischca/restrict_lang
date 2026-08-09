// Arguments come before verbs; adjacent verbs continue the value flow.
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () = {
    // The semicolon stages a named value before a new identifier-led expression.
    val total = (20, 21) add;
    total increment println
}
