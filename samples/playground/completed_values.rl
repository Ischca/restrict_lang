// A completed match stays composable as the next OSV expression's value.
fun show: (value: Int32) -> () = {
    value println
}

fun square: (value: Int32) -> Int32 = {
    value * value
}

fun main: () = {
    true match {
        true => { 6 }
        false => { 2 }
    } square show
}
