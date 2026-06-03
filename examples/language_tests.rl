// v0.0.1 language-surface smoke example.
// Covers records, OSV calls, Option matching, and expression conditionals.

record Counter {
    value: Int32
}

fun increment: (counter: Counter) -> Counter = {
    val Counter { value } = counter;
    Counter { value: value + 1 }
}

fun select_total: (maybe_total: Option<Int32>, fallback: Int32) -> Int32 = {
    maybe_total match {
        Some(total) => { total }
        None => { fallback }
    }
}

fun classify: (value: Int32) -> Int32 = {
    value > 10 then {
        1
    } else {
        0
    }
}

fun main: () -> Int32 = {
    val base = Counter { value: 1 };
    val next = base |> increment;
    val selected = (Some(12), 0) select_total;
    val bucket = selected |> classify;

    next.value + bucket
}
