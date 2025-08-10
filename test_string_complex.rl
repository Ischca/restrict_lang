// Test complex string operations

fun greet: (name: String) = {
    name |> println;
}

fun main: () = {
    "Alice" |> greet;
    "Bob" |> greet;
}