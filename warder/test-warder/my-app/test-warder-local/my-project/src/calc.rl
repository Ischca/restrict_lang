fun add = x:Int y:Int {
    x + y
}

fun main = {
    val result = (10, 20) add
    "Result is: " |> println
    result |> print_int
}