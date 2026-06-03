// Runtime dogfood for generic inference with scalar host ABI.

fun identity_local: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun build_numbers: () -> List<Int32> = {
    val answer = 42 |> identity_local;
    val chosen = (answer, 0) choose_first;
    val numbers: List<Int32> = [chosen, 2, 3, 0];
    val large_numbers = (numbers, |n| n > 1) filter;

    (large_numbers, |n| n * 2) map
}

fun summarize_numbers: (numbers: List<Int32>) -> Int32 = {
    (numbers, 0, |acc, n| acc + n) fold
}

fun generic_pipeline_score: () -> Int32 = {
    val numbers = () build_numbers;

    (numbers |> summarize_numbers) + 3
}

fun main: () -> Int32 = {
    () generic_pipeline_score
}

export fun dogfood_generic_pipeline_runtime_score: () -> Int32 = {
    () generic_pipeline_score
}
