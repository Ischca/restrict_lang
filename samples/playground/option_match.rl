// Built-in options are handled with an exhaustive match.
fun choose: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(number) => { number }
        None => { 0 }
    }
}

fun main: () = {
    42 Option::Some |> choose |> println
}
