// Dogfood release patch inference with clone/freeze.
// Exercises current v0.0.1 record clone updates, freeze, Option/Result routing,
// and expected-type inference for an empty List field.

record ReleaseSnapshot {
    build_id: Int32,
    risk_score: Int32,
    required_checks: Int32,
    passing_checks: Int32,
    hold_owner: Option<Int32>,
    audit_codes: List<Int32>,
    route: Result<Int32, Int32>
}

record ReviewInput {
    checks_delta: Int32,
    new_findings: Int32,
    reviewer: Option<Int32>,
    freeze_rollout: Boolean
}

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun route_for: (risk_score: Int32, freeze_rollout: Boolean) -> Result<Int32, Int32> = {
    freeze_rollout then {
        Err(risk_score)
    } else {
        risk_score <= 20 then {
            Ok(risk_score)
        } else {
            Err(risk_score - 20)
        }
    }
}

fun hold_owner_for: (
    reviewer: Option<Int32>,
    freeze_rollout: Boolean
) -> Option<Int32> = {
    freeze_rollout then {
        reviewer
    } else {
        None
    }
}

fun patch_release: (base: ReleaseSnapshot, review: ReviewInput) = {
    val ReviewInput {
        checks_delta,
        new_findings,
        reviewer,
        freeze_rollout
    } = review;
    val risk_score = new_findings * 9;
    val passing_checks = checks_delta + 3;
    val hold_owner = (reviewer, freeze_rollout) hold_owner_for;
    val route = (risk_score, freeze_rollout) route_for;
    val patched = base.clone {
        risk_score: risk_score,
        passing_checks: passing_checks,
        hold_owner: hold_owner,
        audit_codes: [],
        route: route
    };

    patched freeze
}

fun main: () -> Int32 = {
    val base = ReleaseSnapshot {
        build_id: 77,
        risk_score: 0,
        required_checks: 3,
        passing_checks: 1,
        hold_owner: None,
        audit_codes: [100],
        route: Ok(0)
    };
    val review = ReviewInput {
        checks_delta: 2,
        new_findings: 1,
        reviewer: Some(42),
        freeze_rollout: false
    };
    val patched = (base, review) patch_release;
    val ReleaseSnapshot {
        build_id,
        risk_score,
        required_checks,
        passing_checks,
        hold_owner,
        audit_codes,
        route
    } = patched;
    val audit_score = (audit_codes, 0, add_int) fold;
    val owner_score = hold_owner match {
        Some(owner) => {
            owner
        }
        None => {
            0
        }
    };
    val route_score = route match {
        Ok(score) => {
            score
        }
        Err(shortfall) => {
            0 - shortfall
        }
    };

    build_id + risk_score + required_checks + passing_checks + owner_score + audit_score + route_score
}
