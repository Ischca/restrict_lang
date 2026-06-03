// Dogfoods context bindings as a scoped policy capability.
// The policy values flow through `with`, not through object-style method state.

context ReviewPolicy {
    minimum_score: Float64,
    risk_penalty: Float64
}

fun adjusted_score: (signal: Float64, risk: Float64) -> Float64 = {
    signal - risk
}

fun main: () -> Float64 = {
    val base_score = 0.91;
    val penalty = 0.12;
    with ReviewPolicy { minimum_score: 0.8, risk_penalty: penalty } {
        val score = (base_score, risk_penalty) adjusted_score;
        score >= minimum_score then {
            score
        } else {
            minimum_score
        }
    }
}
