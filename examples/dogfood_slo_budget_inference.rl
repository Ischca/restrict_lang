// Dogfood SLO budget triage for v0.0.1 inference.
// Models service health as value flow: infer generic Option helpers, local
// lambdas through map/filter/fold, record destructuring, and Result routing.

record ServiceSignal {
    service_id: Int32,
    tier: Int32,
    latency_ms: Int32,
    error_rate: Int32,
    owner: Option<Int32>
}

record SloPolicy {
    latency_budget: Int32,
    error_budget: Int32,
    fallback_owner: Int32,
    page_threshold: Int32
}

record SloCandidate {
    service_id: Int32,
    owner: Int32,
    risk: Int32,
    page: Boolean,
    route: Result<Int32, Int32>
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

fun budget_overage: (actual: Int32, budget: Int32) -> Int32 = {
    actual > budget then {
        actual - budget
    } else {
        0
    }
}

fun risk_score: (
    latency_ms: Int32,
    latency_budget: Int32,
    error_rate: Int32,
    error_budget: Int32,
    tier: Int32
) -> Int32 = {
    val latency_over = (latency_ms, latency_budget) budget_overage;
    val error_over = (error_rate, error_budget) budget_overage;

    latency_over + error_over * 4 + tier
}

fun route_candidate: (
    page: Boolean,
    service_id: Int32,
    risk: Int32
) -> Result<Int32, Int32> = {
    page then {
        Ok(service_id)
    } else {
        Err(risk)
    }
}

fun candidate_for: (
    signal: ServiceSignal,
    policy: SloPolicy
) -> SloCandidate = {
    val ServiceSignal {
        service_id,
        tier,
        latency_ms,
        error_rate,
        owner
    } = signal;
    val SloPolicy {
        latency_budget,
        error_budget,
        fallback_owner,
        page_threshold
    } = policy;
    val risk = (latency_ms, latency_budget, error_rate, error_budget, tier) risk_score;
    val owner_id = (owner, fallback_owner) choose_value;
    val page = risk >= page_threshold;
    val route = (page, service_id, risk) route_candidate;

    SloCandidate {
        service_id: service_id,
        owner: owner_id,
        risk: risk,
        page: page,
        route: route
    }
}

fun remember_first_unowned: (
    current: Option<Int32>,
    owner: Option<Int32>,
    service_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => {
            Some(existing)
        }
        None => {
            owner match {
                Some(person) => {
                    None
                }
                None => {
                    Some(service_id)
                }
            }
        }
    }
}

fun is_paged: (candidate: SloCandidate) -> Boolean = {
    val SloCandidate {
        service_id,
        owner,
        risk,
        page,
        route
    } = candidate;

    page
}

fun routed_risk: (candidate: SloCandidate) -> Int32 = {
    val SloCandidate {
        service_id,
        owner,
        risk,
        page,
        route
    } = candidate;

    route match {
        Ok(id) => {
            risk + id
        }
        Err(shortfall) => {
            0 - shortfall
        }
    }
}

fun main: () -> Int32 = {
    val policy = SloPolicy {
        latency_budget: 120,
        error_budget: 2,
        fallback_owner: 404,
        page_threshold: 20
    };
    val signals = [
        ServiceSignal {
            service_id: 11,
            tier: 1,
            latency_ms: 145,
            error_rate: 3,
            owner: Some(7)
        },
        ServiceSignal {
            service_id: 12,
            tier: 3,
            latency_ms: 98,
            error_rate: 1,
            owner: None
        },
        ServiceSignal {
            service_id: 13,
            tier: 2,
            latency_ms: 180,
            error_rate: 5,
            owner: Some(9)
        }
    ];
    val candidates = (signals, |signal| (signal, policy) candidate_for) map;
    val paged = (candidates, |candidate| candidate |> is_paged) filter;
    val routed_risks = (paged, |candidate| candidate |> routed_risk) map;
    val total_risk = (routed_risks, 0, |total, risk| total + risk) fold;
    val first_unowned = (None, None, 12) remember_first_unowned;

    (first_unowned, total_risk) choose_value
}
