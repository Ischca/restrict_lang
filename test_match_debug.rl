fun foo = { 1 }
fun main = {
    match Some(42) {
        Some(n) => n
        None => 0
    }
}