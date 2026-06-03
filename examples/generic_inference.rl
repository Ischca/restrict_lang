// Generic inference with OSV calls and expected lambda context.

fun identity_local: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val answer = 42 |> identity_local;
    val chosen = (answer, 0) choose_first;
    val numbers: List<Int32> = [chosen, 2, 3];
    val large_numbers = (numbers, |n| n > 1) filter;

    (large_numbers, |n| n * 2) map
}
