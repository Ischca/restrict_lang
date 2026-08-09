// A non-Copy binding can be consumed at most once.
fun consume: (value: String) -> () = {
    value println
}

fun main: () = {
    val message = "use me once";
    message consume;
    message consume
}
