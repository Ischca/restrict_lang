// OSV syntax examples
fun adjust_score: (score: Int32) -> Int32 = {
    score + 1
}

fun process_scores: (scores: List<Int32>) -> Int32 = {
    val kept = scores filter { it > 0 }
    val adjusted = kept map { |score| score |> adjust_score }
    (adjusted, 0) fold { |total, score| total + score }
}

// Pipe operator chains
fun pipe_example: () -> Int32 = {
    41
        |> adjust_score
        |> (|score| (score, 10) max)
}
