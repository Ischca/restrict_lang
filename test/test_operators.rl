// Arithmetic operations
val sum = 10 + 20
val diff = 50 - 15
val product = 6 * 7
val quotient = 100 / 4
val remainder = 17 % 5

// Comparisons
val less = 10 < 20
val greater = 30 > 10
val equal = 5 == 5
val not_equal = 10 != 20

// Pipe operations
val x = 42 |> doubled
val y = doubled + 10

// Function with pipe
fun square = n: Int { n * n }
val result = 5 |> square