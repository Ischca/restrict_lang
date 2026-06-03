// Lambda inference is bidirectional.
// Unannotated params need expected context; typed params can provide it locally.

fun apply_to_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun apply_predicate: (f: Int32 -> Boolean, value: Int32) -> Boolean = {
    value |> f
}

fun main: () -> Int32 = {
    val from_context = (|x| x + 1, 40) apply_to_int;
    val bump = |value: Int32| value + 1;
    val incremented = from_context |> bump;
    val positive = (|x| x > 0, incremented) apply_predicate;

    positive match {
        true => { incremented }
        false => { 0 }
    }
}
