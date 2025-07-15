fun multiply = x:Int y:Int {
    x * y
}

fun main = {
    // 簡単なfactorialの例: 5! = 5 * 4 * 3 * 2 * 1 = 120
    val step1 = (2, 1) multiply  // 2
    val step2 = (3, step1) multiply  // 6
    val step3 = (4, step2) multiply  // 24  
    val result = (5, step3) multiply  // 120
    "5! = " |> println
    result |> print_int
}