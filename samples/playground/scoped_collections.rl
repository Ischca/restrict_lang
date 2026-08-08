// A verb can open a typed scope; the complete clause feeds the next value flow.
fun main: () = {
    // A lambda binder names the scoped value without a separate local declaration.
    val shifted = [20, 21] map { |value|
        value + 1
    }
    ((shifted, 0) fold { |total, value|
        total + value
    }) println
}
