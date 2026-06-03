// Practical v0.0.1 support rotation inference dogfood.
// Models a support handoff plan while keeping the exported ABI primitive.

record SupportTicket {
    severity: Int32,
    age_hours: Int32,
    customer_tier: Option<Int32>
}

record RotationPolicy {
    active_window: Range<Int32>,
    load_limit: Int32,
    fallback_owner: Int32,
    manual_owner: Option<Int32>
}

record RotationPlan {
    active_window: Range<Int32>,
    owners: List<Int32>,
    scored_loads: List<Int32>,
    selected_owner: Option<Int32>,
    route: Result<Int32, Int32>,
    audit_codes: List<Int32>,
    sampled_owners: Option<List<Int32>>
}

fun choose_value: <T>(candidate: Option<T>, fallback: T) -> T = {
    candidate match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun ticket_load: (ticket: SupportTicket) -> Int32 = {
    val SupportTicket {
        severity,
        age_hours,
        customer_tier
    } = ticket;
    val tier_bonus = (customer_tier, 0) choose_value;

    (severity * 10) + age_hours + tier_bonus
}

fun add_load: (total: Int32, load: Int32) -> Int32 = {
    total + load
}

fun route_support: (total_load: Int32, limit: Int32, owner: Int32) -> Result<Int32, Int32> = {
    total_load <= limit then {
        Ok(owner)
    } else {
        Err(total_load - limit)
    }
}

fun build_rotation_plan: (policy: RotationPolicy, tickets: List<SupportTicket>) -> RotationPlan = {
    val RotationPolicy {
        active_window,
        load_limit,
        fallback_owner,
        manual_owner
    } = policy;
    val loads = (tickets, ticket_load) map;
    val urgent_loads = (loads, |load| load >= 20) filter;
    val total_load = (urgent_loads, 0, add_load) fold;
    mut val adjusted_limit = load_limit;
    adjusted_limit = adjusted_limit - 5;
    val owner = (manual_owner, fallback_owner) choose_value;
    val route = (total_load, adjusted_limit, owner) route_support;

    RotationPlan {
        active_window: active_window,
        owners: [fallback_owner, owner],
        scored_loads: [],
        selected_owner: Some(owner),
        route: route,
        audit_codes: [],
        sampled_owners: Some([])
    }
}

fun score_rotation: () -> Int32 = {
    val policy = RotationPolicy {
        active_window: [3..8],
        load_limit: 75,
        fallback_owner: 31,
        manual_owner: None
    };
    val tickets = [
        SupportTicket {
            severity: 2,
            age_hours: 5,
            customer_tier: Some(3)
        },
        SupportTicket {
            severity: 1,
            age_hours: 3,
            customer_tier: None
        },
        SupportTicket {
            severity: 3,
            age_hours: 4,
            customer_tier: Some(2)
        }
    ];
    val plan = (policy, tickets) build_rotation_plan;
    val RotationPlan {
        active_window,
        owners,
        scored_loads,
        selected_owner,
        route,
        audit_codes,
        sampled_owners
    } = plan;
    val owner_score = (selected_owner, 0) choose_value;
    val route_score = route match {
        Ok(owner) => {
            owner
        }
        Err(shortfall) => {
            0 - shortfall
        }
    };

    owner_score + route_score
}

export fun support_rotation_score: () -> Int32 = {
    () score_rotation
}
