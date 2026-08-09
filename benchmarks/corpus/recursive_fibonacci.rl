fun fibonacci: (value: Int32) -> Int32 = {
    value <= 1 then {
        value
    } else {
        val first = (value - 1) fibonacci
        val second = (value - 2) fibonacci
        first + second
    }
}

pub fun benchmark: (value: Int32) -> Int32 = {
    value fibonacci
}
