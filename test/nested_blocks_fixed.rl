fun main = {
    val outer = {
        val inner = 10
        val result = inner + 5
        { result }
    }
    val final = outer * 2
    { final }
}