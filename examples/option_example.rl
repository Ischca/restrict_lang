// Example demonstrating Option type usage in Restrict Language

fun main: () = {
    // Basic Option usage
    val x = (42) Option::Some;

    // Pattern matching on Options (checking presence)
    val is_some = x match {
        Some(_) => { 1 }
        None => { 0 }
    };

    // Conditional expressions
    is_some > 0 then {
        42
    } else {
        0
    }
}
