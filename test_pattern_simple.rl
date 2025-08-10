// Simple pattern matching test

fun test: (x: Bool) -> Int32 = {
    x match {
        true => { 1 }
        // Missing false case - should trigger exhaustiveness error
    }
}

fun main: () = {
    val result = true |> test;
    result |> println;
}