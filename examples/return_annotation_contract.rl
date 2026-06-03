// Return annotation contract example.
// Forward references use annotated signatures before function bodies are checked.

fun main: () -> Boolean = {
    41 |> is_positive
}

fun is_positive: (value: Int32) -> Boolean = {
    value > 0
}
