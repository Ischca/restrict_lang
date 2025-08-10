// Simple TAT cleanup test

fun main: () = {
    "Starting TAT test" |> println;
    
    // Test temporal scope without actual temporal types for now
    with lifetime<~io> {
        "Inside temporal scope" |> println;
        val x = 42;
        x |> println;
    }
    
    "Temporal scope ended" |> println;
}