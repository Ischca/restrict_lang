// Basic TAT test - using existing with syntax

fun main: () = {
    "Starting TAT test" |> println;
    
    // Arena scope works with existing syntax
    with Arena {
        "Inside arena scope" |> println;
        val x = 42;
        x |> println;
    }
    
    "Arena scope ended" |> println;
}