// A verb can open a typed scope; the complete clause feeds the next value flow.
fun main: () = {
    val values = [20, 21]
    val shifted = values map {
        it + 1
    }
    (shifted, 0) fold { |total, value|
        total + value
    } |> println
}
