// A context supplies typed capabilities for one lexical body.
context Pricing {
    base: Int32
    bonus: Int32
}

fun main: () = {
    with Pricing { base: 40, bonus: 2 } {
        base + bonus
    } println
}
