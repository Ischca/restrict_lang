// OSV syntax examples
fun adjust_score: (score: Int32) -> Int32 = {
    score + 1
}

fun process_scores: (scores: List<Int32>) -> Int32 = {
    val kept = (scores, |score| score > 0) filter
    val adjusted = (kept, |score| score |> adjust_score) map
    (adjusted, 0, |total, score| total + score) fold
}

// Pipe operator chains
fun pipe_example: () -> Int32 = {
    41
        |> adjust_score
        |> (|score| (score, 10) max)
}
