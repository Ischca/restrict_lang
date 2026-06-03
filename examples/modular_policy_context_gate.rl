// Modular policy-context gate example.
// Dogfoods source imports with module-private context and impl dispatch.

import modules.policy_context.{ReviewSignal, RolloutSignal, decide_review}

export fun modular_policy_context_score: () -> Int32 = {
    val review = ReviewSignal {
        failures: 1,
        stale_days: 3
    };
    val rollout = RolloutSignal {
        exposure_percent: 25,
        canary_failures: 0
    };

    (review, rollout, 80) decide_review
}

fun main: () -> Int32 = {
    () modular_policy_context_score
}
