fun add = a: Int b: Int { a + b }
fun inc = x: Int { x + 1 }

// Simple pipe
val x = 42 |> inc

// Function with two args needs different syntax
val y = 10 add 5

// Or use pipe then apply
val z = 5 |> add
val w = 10 z