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

// Using pipe operator with function calls
fun pipeExample = {
    val x = 10
    val y = 20
    val result = x |> add y
    result
}