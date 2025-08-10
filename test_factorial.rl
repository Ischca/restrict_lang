// Test recursive functions with factorial

fun factorial: (n: Int32) -> Int32 = {
    // Compute n-1 first, then multiply
    val prev = n - 1;
    val result = prev |> factorial;
    n * result
}

fun main: () = {
    5 |> factorial |> println;
}