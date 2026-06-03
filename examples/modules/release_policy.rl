// Release policy module.
// Kept intentionally small so the importing example can exercise source-level
// modules, nested imports, exported records, Option defaults, and list scoring
// together.

import modules.release_scores.{score_signal, add_signal_score}

export record ReleaseInput {
    lead_time_days: Int32,
    failing_checks: Int32,
    risk_signals: List<Int32>,
    manual_owner: Option<Int32>
}

export record ReleaseVerdict {
    score: Int32,
    blocked: Boolean,
    owner: Int32
}

fun choose_owner: (manual_owner: Option<Int32>, default_owner: Int32) -> Int32 = {
    manual_owner match {
        Some(owner_id) => {
            owner_id
        }
        None => {
            default_owner
        }
    }
}

export fun evaluate_release: (input: ReleaseInput, default_owner: Int32) -> ReleaseVerdict = {
    val ReleaseInput {
        lead_time_days,
        failing_checks,
        risk_signals,
        manual_owner
    } = input;
    val signal_scores = (risk_signals, score_signal) map;
    val signal_total = (signal_scores, 0, add_signal_score) fold;
    val score = signal_total + (failing_checks * 10) - lead_time_days;
    val owner = (manual_owner, default_owner) choose_owner;

    ReleaseVerdict {
        score: score,
        blocked: score > 25,
        owner: owner
    }
}
