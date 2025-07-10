fun test_ops = a: Int b: Int {
    val add = 10 + 3
    val sub = 10 - 3
    val mul = 10 * 3
    val div = 10 / 3
    val mod = 10 % 3
    val eq = 10 == 3
    val ne = 10 != 3
    val lt = 10 < 3
    val le = 10 <= 3
    val gt = 10 > 3
    val ge = 10 >= 3
    42
}

fun main = {
    (10, 3) test_ops
}