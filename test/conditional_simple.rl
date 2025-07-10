fun max = a: Int b: Int {
    // Simple version without reusing variables
    10 > 5 then {
        42
    } else {
        73
    }
}

fun main = {
    (0, 0) max
}