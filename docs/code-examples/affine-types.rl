// Affine type examples
fun affineExample = {
    val message = "This can only be used once"

    // First use - OK
    message println

    // Second use - Compile error!
    // message println  // Error: message already consumed
}

// Using clone with records
record Point { x: Int, y: Int }

fun cloneExample = {
    val original = Point { x = 10, y = 20 }
    val copy = original.clone {}

    // Now we can use copy while original is consumed by clone
    copy
}