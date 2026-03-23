// Affine type examples

fun greet: (name: String) -> String = {
    name
}

fun main = {
    val message = "World"

    // First use - OK (consumes message)
    message greet |> println

    // Second use would be a compile error:
    // message greet |> println  // Error: message already consumed
}
