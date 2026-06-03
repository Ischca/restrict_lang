fun add: (a: Int32, b: Int32) -> Int32 = { a + b }

fun main: () -> Int32 = {
    val x = 42
    val y = 10
    val result = (x, y) add
    result
}
