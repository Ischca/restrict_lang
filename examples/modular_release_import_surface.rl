// Whole-module and wildcard release import example.
// Dogfoods the v0.0.1 source import forms that bring exported declarations
// into the current scope without aliases or re-exports.

import modules.release_policy
import modules.release_scores.*

val fallback_owner: Int32 = 7

export fun modular_release_import_surface_score: () -> Int32 = {
    val input = ReleaseInput {
        lead_time_days: 1,
        failing_checks: 2,
        risk_signals: [3, 5],
        manual_owner: Some(99)
    };
    val ReleaseVerdict {
        score,
        blocked,
        owner
    } = (input, fallback_owner) evaluate_release;
    val signal_bonus = 3 |> score_signal;

    score + owner + signal_bonus
}

fun main: () -> Int32 = {
    () modular_release_import_surface_score
}
