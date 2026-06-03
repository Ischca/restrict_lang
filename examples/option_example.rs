// Example demonstrating Option type usage in Restrict Language.

const OPTION_EXAMPLE: &str = r#"
// Option type provides null safety in current Restrict syntax.
fun unwrap_or: (opt: Option<Int32>, default: Int32) -> Int32 = {
    opt match {
        Some(n) => { n }
        None => { default }
    }
}

fun find_first_even: (start: Int32, end_val: Int32) -> Option<Int32> = {
    start > end_val then {
        None
    } else {
        start % 2 == 0 then {
            Some(start)
        } else {
            (start + 1, end_val) find_first_even
        }
    }
}

fun main: () = {
    // Basic Option usage
    val x = Some(42)
    val y = None

    // Unwrapping with default
    val result1 = (x, 0) unwrap_or
    val result2 = (y, 0) unwrap_or

    // Finding first even number
    val first_even = (1, 10) find_first_even
    first_even match {
        Some(n) => { n |> print_int }
        None => { "No even number found" |> println }
    }

    // Nested Options
    val nested = Some(Some(100))
    nested match {
        Some(inner) => {
            inner match {
                Some(v) => { v }
                None => { 0 }
            }
        }
        None => { 0 }
    }
}
"#;

fn main() {
    println!("{OPTION_EXAMPLE}")
}
