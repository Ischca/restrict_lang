// Example demonstrating Option type usage in Restrict Language

const OPTION_EXAMPLE: &str = r#"
// Option type provides null safety
fun unwrap_or = opt: Option<Int> default: Int {
    opt match {
        Some(n) => { n }
        None => { default }
    }
}

fun find_first_even = start: Int end_val: Int {
    mut val i = start;
    i > end_val then {
        None
    } else {
        i % 2 == 0 then {
            Some(i)
        } else {
            (i + 1, end_val) find_first_even
        }
    }
}

fun main = {
    // Basic Option usage
    val x = Some(42);
    val y = None;
    
    // Unwrapping with default
    val result1 = x unwrap_or 0;  // Should be 42
    val result2 = y unwrap_or 0;  // Should be 0
    
    // Finding first even number
    val first_even = (1, 10) find_first_even;
    first_even match {
        Some(n) => { n println }
        None => { "No even number found" println }
    };
    
    // Nested Options
    val nested = Some(Some(100));
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
    println!("{}", OPTION_EXAMPLE);
}