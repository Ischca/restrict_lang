// Test function with Unit return type (no explicit return type)
fun print_hello: () = {
    "Hello" |> println;
}

// Test function with explicit Unit return type
fun print_world: () -> Unit = {
    "World" |> println;
}

// Test function that ends with while loop (should return Unit)
fun count_to: (n: Int) = {
    mut val i = 1;
    i <= n while {
        i int_to_string |> println;
        i = i + 1
    };
}

fun main: () = {
    print_hello;
    print_world;
    3 count_to;
}
