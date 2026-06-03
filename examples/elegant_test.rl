// v0.0.1 behavior-spec smoke example.
// Keeps the readable spec intent without unsupported fluent matcher syntax.

record Spec {
    name: String,
    passed: Boolean,
    weight: Int32
}

fun spec_score: (spec: Spec) -> Int32 = {
    val Spec { name, passed, weight } = spec;

    passed match {
        true => { weight }
        false => { 0 }
    }
}

fun make_spec: (name: String, passed: Boolean, weight: Int32) -> Spec = {
    Spec {
        name: name,
        passed: passed,
        weight: weight
    }
}

fun total_score: (left: Spec, right: Spec) -> Int32 = {
    val left_score = left |> spec_score;
    val right_score = right |> spec_score;

    left_score + right_score
}

fun main: () -> Int32 = {
    val addition = ("addition stays correct", 1 + 1 == 2, 2) make_spec;
    val ordering = ("ordering stays correct", 5 > 3, 3) make_spec;

    (addition, ordering) total_score
}
