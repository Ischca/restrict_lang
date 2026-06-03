// Dogfood for the intentional v0.0.1 generic export ABI gap.
// This models the API a release planner would want to expose to host code:
// a generic override selector that works for owner IDs and check lists.
// Built-in Option and Result remain supported here. The program should parse
// and type-check, but codegen rejects the exported generic function until
// Restrict has a concrete WebAssembly ABI design for exported generics.

record ReleaseCandidate {
    build_id: Int32,
    risk_score: Int32,
    owner_override: Option<Int32>,
    check_override: Option<List<Int32>>,
    default_checks: List<Int32>
}

record PublishDecision {
    build_id: Int32,
    owner_id: Int32,
    check_ids: List<Int32>,
    route: Result<Int32, Int32>
}

pub fun select_override: <T>(override_value: Option<T>, fallback: T) -> T = {
    override_value match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun route_release: (risk_score: Int32, owner_id: Int32) -> Result<Int32, Int32> = {
    risk_score < 50 then {
        Ok(owner_id)
    } else {
        Err(risk_score)
    }
}

fun plan_publish: (candidate: ReleaseCandidate) -> PublishDecision = {
    val ReleaseCandidate {
        build_id,
        risk_score,
        owner_override,
        check_override,
        default_checks
    } = candidate;
    val owner_id = (owner_override, 42) select_override;
    val check_ids = (check_override, default_checks) select_override;
    val route = (risk_score, owner_id) route_release;

    PublishDecision {
        build_id: build_id,
        owner_id: owner_id,
        check_ids: check_ids,
        route: route
    }
}

fun main: () -> PublishDecision = {
    val candidate = ReleaseCandidate {
        build_id: 1001,
        risk_score: 27,
        owner_override: Some(314),
        check_override: None,
        default_checks: [10, 20, 30]
    };

    candidate |> plan_publish
}
