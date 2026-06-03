// v0.0.1 compact-check smoke example.
// Avoids custom infix operators and removed conditional syntax.

record Check {
    value: Int32,
    limit: Int32
}

record CheckSummary {
    passed: Boolean,
    score: Int32
}

fun above_limit: (check: Check) -> Boolean = {
    val Check { value, limit } = check;
    value > limit
}

fun score_check: (check: Check) -> CheckSummary = {
    val Check { value, limit } = check;
    val passed = value > limit;

    CheckSummary {
        passed: passed,
        score: value - limit
    }
}

fun merge_summary: (left: CheckSummary, right: CheckSummary) -> CheckSummary = {
    val CheckSummary { passed: left_passed, score: left_score } = left;
    val CheckSummary { passed: right_passed, score: right_score } = right;

    CheckSummary {
        passed: left_passed && right_passed,
        score: left_score + right_score
    }
}

fun main: () -> Int32 = {
    val first = Check { value: 9, limit: 4 };
    val second = Check { value: 7, limit: 3 };
    val first_summary = first |> score_check;
    val second_summary = second |> score_check;
    val summary = (first_summary, second_summary) merge_summary;

    summary.score
}
