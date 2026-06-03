// Generic function value pipeline example.
// Dogfoods user-defined generic function values, Float64 specialization,
// and higher-order generic calls where the lambda appears before the value.

record ScoreSummary {
    normalized: List<Float64>,
    first_score: Float64,
    adjusted_score: Float64
}

fun keep: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun empty_values: <T>() -> List<T> = {
    []
}

fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun main: () -> ScoreSummary = {
    val raw_scores = [1.5, 2.5, 3.5];
    val fallback_scores: List<Float64> = () empty_values;
    val source_scores = (raw_scores, fallback_scores) choose_first;
    val normalized = (source_scores, keep) map;
    val first_score = 1.5 |> keep;
    val adjusted_score = (|score| score + 0.25, first_score) apply_first;

    ScoreSummary {
        normalized: normalized,
        first_score: first_score,
        adjusted_score: adjusted_score
    }
}
