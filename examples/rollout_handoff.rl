// Rollout handoff example.
// A small release handoff policy that exercises inferred record-returning
// helpers, field access on inferred call results, and record-based decisions.

record RolloutSignal {
    id: Int32,
    region: String,
    error_budget: Int32,
    canary_passed: Boolean
}

record RolloutDecision {
    risk_score: Int32,
    page: Boolean,
    lane: String
}

fun keep_signal: (signal: RolloutSignal) = {
    val selected = signal;
    selected
}

fun forwarded_budget: (signal: RolloutSignal) -> Int32 = {
    (signal |> keep_signal).error_budget
}

fun decide: (signal: RolloutSignal) = {
    val RolloutSignal { id, region, error_budget, canary_passed } = signal;
    val canary_penalty = canary_passed match {
        true => {
            0
        }
        false => {
            20
        }
    };
    val score = error_budget + canary_penalty;

    RolloutDecision {
        risk_score: score,
        page: score > 30,
        lane: region
    }
}

fun main: () -> Int32 = {
    val budget_signal = RolloutSignal {
        id: 42,
        region: "global",
        error_budget: 18,
        canary_passed: true
    };
    val budget = budget_signal |> forwarded_budget;

    val decision_signal = RolloutSignal {
        id: 43,
        region: "regional",
        error_budget: 25,
        canary_passed: false
    };
    val decision = decision_signal |> decide;

    budget + decision.risk_score
}
