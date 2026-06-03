// Modular release gate example.
// Dogfoods source imports with a small app-shaped release decision flow.

import modules.release_policy.{ReleaseInput, ReleaseVerdict, evaluate_release}

val default_owner: Int32 = 42

fun build_modular_release_gate: () -> ReleaseVerdict = {
    val input = ReleaseInput {
        lead_time_days: 3,
        failing_checks: 1,
        risk_signals: [4, 2, 1],
        manual_owner: None
    };

    (input, default_owner) evaluate_release
}

fun main: () -> ReleaseVerdict = {
    () build_modular_release_gate
}

export fun modular_release_gate_score: () -> Int32 = {
    val verdict = () build_modular_release_gate;
    val ReleaseVerdict {
        score,
        blocked,
        owner
    } = verdict;
    val blocked_score = blocked then {
        100
    } else {
        0
    };

    score + owner + blocked_score
}
