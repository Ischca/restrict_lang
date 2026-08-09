fun apply: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform
}

fun main: () -> () = {
    val answer = 41 apply {
        it + 1
    }
    val values = [1, 2, 3]
    val selected = values map {
        it + 1
    } filter {
        it > 2
    }
    val total = (selected, 0) fold { |sum, value|
        sum + value
    }
    (answer + total) |> println
}
