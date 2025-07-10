fun max = a: Int b: Int {
    a > b then {
        a
    } else {
        b
    }
}

fun main = {
    val x = 42
    val y = 73
    val result = (x, y) max
    result
}