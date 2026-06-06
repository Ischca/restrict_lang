// Pipe Operators & OSV Syntax
//
// Restrict uses Object-Subject-Verb word order:
//   "5 double" instead of "double(5)"
//
// The pipe operator |> makes data flow explicit
// and chains transformations left-to-right.

fun double: (n: Int32) -> Int32 = {
    n * 2
}

fun add_one: (n: Int32) -> Int32 = {
    n + 1
}

fun main: () -> Int32 = {
    // OSV syntax: object comes first, then the function
    val a = 5 |> double       // => 10

    // Chaining with pipes: read left-to-right
    val b = 10 |> double |> add_one  // 10 -> 20 -> 21

    // Pipes compose naturally into a pipeline
    b + (5 |> double |> double)  // => 41
}
