// Complete pattern matching test

fun boolToInt: (x: Bool) -> Int32 = {
    x match {
        true => { 1 }
        false => { 0 }
    }
}

fun main: () = {
    val t = true |> boolToInt;
    val f = false |> boolToInt;
    
    t |> println;
    f |> println;
}