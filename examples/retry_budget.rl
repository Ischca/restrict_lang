// Retry budget example.
// Dogfoods mutable bindings, assignment, comparison, arithmetic, and while loops.

fun consume_retry_budget: (max_attempts: Int32, initial_budget: Int32) -> Int32 = {
    mut val attempts = 0;
    mut val budget = initial_budget;

    (attempts < max_attempts) while {
        budget = budget - 1
        attempts = attempts + 1
    }

    budget
}

fun main: () -> Int32 = {
    (3, 5) consume_retry_budget
}
