fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () -> () = {
    val result = (10, 20) add;
    result |> print_int
}
