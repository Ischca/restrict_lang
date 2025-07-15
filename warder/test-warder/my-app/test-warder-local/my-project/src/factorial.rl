fun factorial = n:Int {
    var mut n = n
    n <= 1 then {
        1
    } else {
        n * (n - 1 |> factorial)
    }
}

fun main = {
    val result = 5 |> factorial
    "Factorial of 5 is: " |> println
    result |> print_int
}