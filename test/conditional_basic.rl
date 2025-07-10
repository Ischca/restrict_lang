fun is_positive = x: Int {
    x > 0 then {
        1
    } else {
        0
    }
}

fun main = {
    val result = (42) is_positive
    result
}