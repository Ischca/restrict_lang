// Review policy factory example.
// Dogfoods curried policy construction with nested closures, record input,
// conditional scoring, and OSV function application.

record Change {
    risk: Int32,
    impact: Int32,
    tests_added: Int32,
    owner_confirmed: Boolean
}

record ReviewDecision {
    score: Int32,
    block_release: Boolean,
    reviewer_lane: Int32
}

fun bool_points: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun test_penalty: (tests_added: Int32) -> Int32 = {
    tests_added < 2 then {
        20
    } else {
        0
    }
}

fun reviewer_lane_for: (score: Int32) -> Int32 = {
    score >= 90 then {
        3
    } else {
        score >= 70 then {
            2
        } else {
            1
        }
    }
}

fun make_policy: (risk_weight: Int32) -> Int32 -> Change -> Int32 = {
    val with_impact: Int32 -> Change -> Int32 = |impact_weight| {
        val score_change: Change -> Int32 = |change| {
            val Change {
                risk,
                impact,
                tests_added,
                owner_confirmed
            } = change;

            val risk_score = risk * risk_weight;
            val impact_score = impact * impact_weight;
            val owner_score = (owner_confirmed, 10) bool_points;
            val missing_tests = tests_added |> test_penalty;

            risk_score + impact_score + owner_score - missing_tests
        };

        score_change
    };

    with_impact
}

fun decide_review: (policy: Change -> Int32, change: Change) -> ReviewDecision = {
    val score = change |> policy;

    ReviewDecision {
        score: score,
        block_release: score >= 100,
        reviewer_lane: score |> reviewer_lane_for
    }
}

fun main: () -> ReviewDecision = {
    val impact_policy = 12 |> make_policy;
    val score_policy = 7 |> impact_policy;
    val change = Change {
        risk: 6,
        impact: 5,
        tests_added: 1,
        owner_confirmed: true
    };

    (score_policy, change) decide_review
}
