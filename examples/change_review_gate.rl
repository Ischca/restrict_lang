// Change review gate example.
// Dogfoods Option as a Container through map/filter while keeping the
// deployment-review flow small enough to serve as a regression example.

record Change {
    id: Int32,
    base_risk: Int32,
    has_migration: Boolean,
    canary_signal: Option<Int32>
}

record ReviewPlan {
    change_id: Int32,
    score: Int32,
    escalation: Option<Int32>,
    accepted: Boolean
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun clamp_signal: (signal: Int32) -> Int32 = {
    signal > 100 then {
        100
    } else {
        signal < 0 then {
            0
        } else {
            signal
        }
    }
}

fun assess_change: (change: Change) -> ReviewPlan = {
    val Change {
        id,
        base_risk,
        has_migration,
        canary_signal
    } = change;
    val migration_points = (has_migration, 15) points_when;
    val normalized_canary = (canary_signal, |signal| signal |> clamp_signal) map;
    val risky_canary = (normalized_canary, |signal| signal >= 80) filter;
    val score = base_risk + migration_points;
    val escalation: Option<Int32> = risky_canary match {
        Some(signal) => {
            Some(id + signal)
        }
        None => {
            score >= 75 then {
                Some(id)
            } else {
                None
            }
        }
    };

    ReviewPlan {
        change_id: id,
        score: score,
        escalation: escalation,
        accepted: score < 75
    }
}

fun main: () -> ReviewPlan = {
    val change = Change {
        id: 42,
        base_risk: 55,
        has_migration: true,
        canary_signal: Some(120)
    };

    change |> assess_change
}
