// Practical v0.0.1 type-inference dogfood.
// Models a small review gate using OSV calls, records, Option, Result,
// generic functions, list pipelines, lambdas, record destructuring, and
// expected-type record fields for empty List/Option sidecars.

record ReviewSignal {
    severity: Int32,
    confidence: Int32,
    reviewer: Option<Int32>
}

record ReviewBatch {
    signals: List<ReviewSignal>,
    manual_override: Option<Int32>,
    fallback_owner: Int32,
    launch_risk_limit: Int32
}

record ReviewSummary {
    owner: Int32,
    risk: Int32,
    approved: Boolean,
    escalation: Result<Int32, Int32>
}

record ReviewAuditSidecar {
    reviewer_notes: List<Int32>,
    skipped_reviewer: Option<Int32>,
    sampled_signal_ids: Option<List<Int32>>
}

fun choose_value: <T>(preferred: Option<T>, fallback: T) -> T = {
    preferred match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun reviewer_score: (reviewer: Option<Int32>) -> Int32 = {
    (reviewer, 0) choose_value
}

fun score_signal: (signal: ReviewSignal) -> Int32 = {
    val ReviewSignal {
        severity,
        confidence,
        reviewer
    } = signal;
    val reviewer_bonus = reviewer |> reviewer_score;

    (severity * confidence) + reviewer_bonus
}

fun add_risk: (total: Int32, risk: Int32) -> Int32 = {
    total + risk
}

fun escalation_code: (approved: Boolean, owner: Int32) -> Result<Int32, Int32> = {
    approved then {
        Ok(owner)
    } else {
        Err(503)
    }
}

fun default_audit_sidecar: () -> ReviewAuditSidecar = {
    ReviewAuditSidecar {
        reviewer_notes: [],
        skipped_reviewer: None,
        sampled_signal_ids: Some([])
    }
}

fun summarize_review: (batch: ReviewBatch) -> ReviewSummary = {
    val ReviewBatch {
        signals,
        manual_override,
        fallback_owner,
        launch_risk_limit
    } = batch;
    val risks = (signals, score_signal) map;
    val meaningful_risks = (risks, |risk| risk > 0) filter;
    val total_risk = (meaningful_risks, 0, add_risk) fold;
    val owner = (manual_override, fallback_owner) choose_value;
    val approved = total_risk < launch_risk_limit;
    val escalation = (approved, owner) escalation_code;

    ReviewSummary {
        owner: owner,
        risk: total_risk,
        approved: approved,
        escalation: escalation
    }
}

fun main: () -> ReviewSummary = {
    val signals: List<ReviewSignal> = [
        ReviewSignal {
            severity: 3,
            confidence: 4,
            reviewer: Some(7)
        },
        ReviewSignal {
            severity: 1,
            confidence: 2,
            reviewer: None
        }
    ];
    val batch = ReviewBatch {
        signals: signals,
        manual_override: None,
        fallback_owner: 42,
        launch_risk_limit: 30
    };

    batch |> summarize_review
}
