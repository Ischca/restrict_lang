// Lambda expected-type inference example.
// Lambdas are inferred from an explicit function context, not guessed from body use.

fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    val add_tax: Int32 -> Int32 = |subtotal| (subtotal * 108) / 100;
    val taxed = 100 |> add_tax;
    (|x| x * 2, taxed) apply_int
}
