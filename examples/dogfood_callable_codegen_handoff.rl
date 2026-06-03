// Dogfoods codegen handoff for inferred callable values.
// Immediate branch expressions produce function values directly as pipe targets,
// including an Option match arm that binds an existing function value.

fun launch_bonus: (score: Int32) -> Int32 = {
    score + 11
}

fun risk_double: (score: Int32) -> Int32 = {
    score * 2
}

fun subtract_guardrail: (score: Int32) -> Int32 = {
    score - 3
}

fun immediate_then_score: (base: Int32, urgent: Boolean) -> Int32 = {
    base |> (urgent then {
        launch_bonus
    } else {
        |score| score + 4
    })
}

fun immediate_match_score: (base: Int32, reviewer_ready: Boolean) -> Int32 = {
    base |> (reviewer_ready match {
        true => {
            |score| score * 3
        }
        false => {
            risk_double
        }
    })
}

fun option_handoff_score: (
    base: Int32,
    override_mapper: Option<Int32 -> Int32>
) -> Int32 = {
    base |> (override_mapper match {
        Some(mapper) => {
            mapper
        }
        None => {
            |score| score + 7
        }
    })
}

fun main: () -> Int32 = {
    val urgent_score = (10, true) immediate_then_score;
    val review_score = (4, false) immediate_match_score;
    val override_mapper: Option<Int32 -> Int32> = Some(subtract_guardrail);
    val override_score = (20, override_mapper) option_handoff_score;
    val fallback_mapper: Option<Int32 -> Int32> = None;
    val fallback_score = (5, fallback_mapper) option_handoff_score;

    urgent_score + review_score + override_score + fallback_score
}
