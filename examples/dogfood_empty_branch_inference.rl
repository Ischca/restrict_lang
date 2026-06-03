// Dogfood branch-heavy empty inference example.
// Models an incident review workflow with inferred [] and None values flowing
// through nested branches, Option matching, Result routing, and list folding.

record ReviewRequest {
    ticket_id: Int32,
    customer_tier: Int32,
    failed_checks: Int32,
    manual_owner: Option<Int32>,
    freeze_rollout: Boolean
}

record ReviewPlan {
    ticket_id: Int32,
    reviewer: Option<Int32>,
    audit_codes: List<Int32>,
    deferred_codes: Option<List<Int32>>,
    route: Result<Int32, Int32>
}

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun risk_score: (customer_tier: Int32, failed_checks: Int32) -> Int32 = {
    customer_tier * 20 + failed_checks * 11
}

fun resolve_reviewer: (
    manual_owner: Option<Int32>,
    risk: Int32,
    freeze_rollout: Boolean,
    fallback_owner: Int32
) -> Option<Int32> = {
    freeze_rollout then {
        None
    } else {
        manual_owner match {
            Some(owner) => {
                Some(owner)
            }
            None => {
                risk >= 75 then {
                    Some(fallback_owner)
                } else {
                    None
                }
            }
        }
    }
}

fun audit_codes_for: (
    risk: Int32,
    customer_tier: Int32,
    freeze_rollout: Boolean
) -> List<Int32> = {
    freeze_rollout then {
        []
    } else {
        risk >= 90 then {
            [900, customer_tier]
        } else {
            customer_tier >= 3 then {
                [300, risk]
            } else {
                []
            }
        }
    }
}

fun deferred_codes_for: (
    freeze_rollout: Boolean,
    risk: Int32
) -> Option<List<Int32>> = {
    freeze_rollout then {
        Some([])
    } else {
        risk >= 90 then {
            Some([risk, 90])
        } else {
            None
        }
    }
}

fun route_for: (risk: Int32, freeze_rollout: Boolean) -> Result<Int32, Int32> = {
    freeze_rollout then {
        Err(risk)
    } else {
        risk >= 80 then {
            Ok(risk)
        } else {
            Err(80 - risk)
        }
    }
}

fun build_plan: (request: ReviewRequest, fallback_owner: Int32) -> ReviewPlan = {
    val ReviewRequest {
        ticket_id,
        customer_tier,
        failed_checks,
        manual_owner,
        freeze_rollout
    } = request;
    val risk = (customer_tier, failed_checks) risk_score;
    val reviewer = (manual_owner, risk, freeze_rollout, fallback_owner) resolve_reviewer;
    val audit_codes = (risk, customer_tier, freeze_rollout) audit_codes_for;
    val deferred_codes = (freeze_rollout, risk) deferred_codes_for;
    val route = (risk, freeze_rollout) route_for;

    ReviewPlan {
        ticket_id: ticket_id,
        reviewer: reviewer,
        audit_codes: audit_codes,
        deferred_codes: deferred_codes,
        route: route
    }
}

fun main: () -> Int32 = {
    val request = ReviewRequest {
        ticket_id: 42,
        customer_tier: 4,
        failed_checks: 2,
        manual_owner: None,
        freeze_rollout: false
    };
    val plan = (request, 77) build_plan;
    val ReviewPlan {
        ticket_id,
        reviewer,
        audit_codes,
        deferred_codes,
        route
    } = plan;
    val audit_score = (audit_codes, 0, add_int) fold;
    val reviewer_score = reviewer match {
        Some(owner) => {
            owner
        }
        None => {
            0
        }
    };
    val deferred_score = deferred_codes match {
        Some(codes) => {
            (codes, 0, add_int) fold
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

    ticket_id + audit_score + reviewer_score + deferred_score + route_score
}
