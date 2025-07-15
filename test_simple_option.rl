fun main = {
    val x = 42 some;
    val result = x match {
        Some(n) => { n }
        None => { 0 }
    };
    result
}