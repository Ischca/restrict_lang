import lib.Math.{add, multiply}

fun main = {
    val sum = add(10, 20);
    val product = multiply(sum, 2);
    product |> println
}