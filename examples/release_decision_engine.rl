// Release decision engine example.
// Dogfoods an app-shaped decision flow with top-level constants, nested record
// destructuring, Option defaults, empty-list field inference, and higher-order
// list scoring.

val default_owner_id: Int32 = 42
val default_risk_limit: Float64 = 75.0

record Signal {
    severity: Float64,
    confidence: Float64,
    owner: Option<Int32>
}

record GateConfig {
    risk_limit: Float64,
    owner_weight: Float64
}

record ReleasePlan {
    scoring_signals: List<Signal>,
    audit_signals: List<Signal>,
    blocker_codes: List<Int32>,
    override_owner: Option<Int32>,
    config: GateConfig
}

record ReleaseDecision {
    risk: Float64,
    blocked: Boolean,
    primary_owner: Int32,
    first_blocker: Int32,
    audit_scores: List<Float64>
}

fun add_score: (total: Float64, score: Float64) -> Float64 = {
    total + score
}

fun has_positive_score: (score: Float64) -> Boolean = {
    score > 0.0
}

fun score_signal: (signal: Signal, owner_weight: Float64) -> Float64 = {
    val Signal {
        severity,
        confidence,
        owner
    } = signal;
    val owner_bonus = owner match {
        Some(owner_id) => {
            owner_id > 0 then {
                owner_weight
            } else {
                0.0
            }
        }
        None => {
            0.0
        }
    };

    (severity * confidence) + owner_bonus
}

fun select_owner: (override_owner: Option<Int32>, fallback_owner: Int32) -> Int32 = {
    override_owner match {
        Some(owner_id) => {
            owner_id
        }
        None => {
            fallback_owner
        }
    }
}

fun first_blocker: (codes: List<Int32>) -> Int32 = {
    codes match {
        [head | tail] => {
            head
        }
        [] => {
            0
        }
    }
}

fun decide_release: (plan: ReleasePlan) -> ReleaseDecision = {
    val ReleasePlan {
        scoring_signals,
        audit_signals,
        blocker_codes,
        override_owner,
        config: GateConfig {
            risk_limit,
            owner_weight
        }
    } = plan;
    val scoring_scores = (scoring_signals, |signal| (signal, owner_weight) score_signal) map;
    val positive_scores = (scoring_scores, has_positive_score) filter;
    val risk = (positive_scores, 0.0, add_score) fold;
    val audit_scores = (audit_signals, |signal| (signal, owner_weight) score_signal) map;
    val primary_owner = (override_owner, default_owner_id) select_owner;
    val first_blocker = blocker_codes |> first_blocker;

    ReleaseDecision {
        risk: risk,
        blocked: risk > risk_limit,
        primary_owner: primary_owner,
        first_blocker: first_blocker,
        audit_scores: audit_scores
    }
}

fun main: () -> ReleaseDecision = {
    val config = GateConfig {
        risk_limit: default_risk_limit,
        owner_weight: 1.25
    };
    val scoring_signals: List<Signal> = [
        Signal {
            severity: 30.0,
            confidence: 0.9,
            owner: Some(7)
        },
        Signal {
            severity: 12.0,
            confidence: 0.4,
            owner: None
        }
    ];
    val audit_signals: List<Signal> = [
        Signal {
            severity: 8.0,
            confidence: 0.5,
            owner: Some(9)
        }
    ];
    val plan = ReleasePlan {
        scoring_signals: scoring_signals,
        audit_signals: audit_signals,
        blocker_codes: [],
        override_owner: None,
        config: config
    };

    plan |> decide_release
}
