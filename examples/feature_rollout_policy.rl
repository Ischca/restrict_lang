// Feature rollout policy example.
// A small feature-gate decision flow that exercises record field patterns,
// rest destructuring, inferred record returns, and field access on inferred
// function call results.

record FeatureSignal {
    feature_id: Int32,
    enabled: Boolean,
    sampled: Boolean,
    error_budget: Int32
}

record FeatureDecision {
    priority: Int32,
    action: Int32,
    notify: Boolean
}

fun sample_penalty: (sampled: Boolean) -> Int32 = {
    sampled match {
        true => { 5 }
        false => { 0 }
    }
}

fun classify_feature: (signal: FeatureSignal) = {
    signal match {
        FeatureSignal { enabled: true, sampled, error_budget, ..._ } => {
            val penalty = sampled |> sample_penalty;
            val priority = error_budget + penalty;

            FeatureDecision {
                priority: priority,
                action: 1,
                notify: priority > 20
            }
        }
        FeatureSignal { enabled: false, ..._ } => {
            FeatureDecision {
                priority: 0,
                action: 0,
                notify: false
            }
        }
    }
}

fun decision_priority: (signal: FeatureSignal) -> Int32 = {
    (signal |> classify_feature).priority
}

fun main: () -> Int32 = {
    val live_signal = FeatureSignal {
        feature_id: 7,
        enabled: true,
        sampled: true,
        error_budget: 18
    };
    val live_priority = live_signal |> decision_priority;

    val paused_signal = FeatureSignal {
        feature_id: 8,
        enabled: false,
        sampled: false,
        error_budget: 3
    };
    val paused = paused_signal |> classify_feature;

    live_priority + paused.priority
}
