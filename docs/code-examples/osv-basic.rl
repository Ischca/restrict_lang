// OSV syntax examples

fun double: (n: Int) -> Int = {
    n * 2
}

fun add_one: (n: Int) -> Int = {
    n + 1
}

// Using pipe operator for left-to-right data flow
fun main = {
    val x = 10
    val result = x |> double |> add_one
    result int_to_string |> println
}
