// OSV syntax examples

// Basic OSV: Object-Subject-Verb order
fun basicOSV = {
    val message = "Hello, World!"
    message println  // Object Subject Verb
}

// Function with parameters
fun add: (a: Int, b: Int) -> Int = {
    a + b
}

// Single-argument function for pipe example
fun double: (n: Int) -> Int = {
    n + n
}

// Using pipe operator: single argument
fun pipeExample = {
    val x = 10
    val result = x |> double
    result
}

// Using pipe operator: tuple auto-expansion for multiple arguments
fun pipeTupleExample = {
    val result = (5, 3) |> add
    result
}
