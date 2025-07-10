record A { value: Int }
record B { value: Int }

impl A {
    fun process = self: A { 100 }
}

impl B {
    fun process = self: B { 200 }
}

fun main = {
    (A { value = 1 }) process + (B { value = 2 }) process
}