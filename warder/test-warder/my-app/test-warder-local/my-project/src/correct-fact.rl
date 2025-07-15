fun factorial = n:Int {
    mut val x = n
    x <= 1 then {
        1
    } else {
        x * (x - 1 |> factorial)
    }
}

fun main = {
    val result = 5 |> factorial
    "Factorial of 5 is: " |> println
    result |> print_int
}