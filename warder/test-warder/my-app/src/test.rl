fun add = x:Int y:Int {
    x + y
}

fun main = {
    val result = (10, 20) add
    result |> print_int
}