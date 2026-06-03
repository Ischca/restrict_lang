// Dogfood support escalation inference example.
// Practical non-TAT workflow for v0.0.1: OSV calls, Option/Result/List flow,
// expected lambdas through map/filter/fold, record destructuring, and explicit
// moves of affine List/record values.

record SupportTicket {
    id: Int32,
    severity: Int32,
    wait_minutes: Int32,
    tier: Int32,
    owner: Option<Int32>,
    tags: List<Int32>,
    reopen_count: Int32
}

record EscalationPolicy {
    wait_limit: Int32,
    severity_threshold: Int32,
    vip_tier: Int32,
    fallback_owner: Int32,
    audit_threshold: Int32
}

record SupportCandidate {
    id: Int32,
    owner: Int32,
    score: Int32,
    tag_score: Int32,
    urgent: Boolean,
    route: Result<Int32, Int32>
}

record SupportState {
    total_score: Int32,
    urgent_count: Int32,
    stale_count: Int32,
    first_unowned: Option<Int32>,
    escalation_ids: List<Int32>,
    audit_notes: List<Int32>
}

record EscalationPlan {
    total_score: Int32,
    urgent_count: Int32,
    stale_count: Int32,
    first_unowned: Option<Int32>,
    candidates: List<SupportCandidate>,
    escalation_ids: List<Int32>,
    audit_notes: List<Int32>,
    deferred_owner: Option<Int32>,
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

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => {
            points
        }
        false => {
            0
        }
    }
}

fun move_ticket_stream: (tickets: List<SupportTicket>) -> List<SupportTicket> = {
    tickets
}

fun move_ticket: (ticket: SupportTicket) -> SupportTicket = {
    ticket
}

fun move_audit_notes: (notes: List<Int32>) -> List<Int32> = {
    notes
}

fun owner_or_default: (owner: Option<Int32>, fallback_owner: Int32) -> Int32 = {
    val active_owner = (owner, |person| person > 0) filter;
    val normalized_owner = (active_owner, |person| person + 0) map;

    (normalized_owner, fallback_owner) choose_value
}

fun wait_overage: (wait_minutes: Int32, wait_limit: Int32) -> Int32 = {
    wait_minutes > wait_limit then {
        wait_minutes - wait_limit
    } else {
        0
    }
}

fun sum_tags: (tags: List<Int32>) -> Int32 = {
    (tags, 0, |total, tag| total + tag) fold
}

fun score_fields: (
    severity: Int32,
    wait_minutes: Int32,
    wait_limit: Int32,
    tier: Int32,
    vip_tier: Int32,
    tag_score: Int32,
    reopen_count: Int32
) -> Int32 = {
    val wait_points = (wait_minutes, wait_limit) wait_overage;
    val vip_points = (tier == vip_tier, 25) points_when;

    severity * 12 + wait_points + vip_points + tag_score + reopen_count * 9
}

fun route_ticket: (
    urgent: Boolean,
    ticket_id: Int32,
    score: Int32
) -> Result<Int32, Int32> = {
    urgent then {
        Ok(ticket_id)
    } else {
        Err(score)
    }
}

fun route_plan: (urgent_count: Int32, total_score: Int32) -> Result<Int32, Int32> = {
    urgent_count > 0 then {
        Ok(total_score)
    } else {
        Err(total_score)
    }
}

fun should_escalate: (
    ticket: SupportTicket,
    wait_limit: Int32,
    severity_threshold: Int32,
    vip_tier: Int32
) -> Boolean = {
    val SupportTicket {
        id,
        severity,
        wait_minutes,
        tier,
        owner,
        tags,
        reopen_count
    } = ticket;
    val tag_score = tags |> sum_tags;
    val score = (
        severity,
        wait_minutes,
        wait_limit,
        tier,
        vip_tier,
        tag_score,
        reopen_count
    ) score_fields;

    score >= severity_threshold
}

fun candidate_for: (
    ticket: SupportTicket,
    wait_limit: Int32,
    severity_threshold: Int32,
    vip_tier: Int32,
    fallback_owner: Int32
) -> SupportCandidate = {
    val SupportTicket {
        id,
        severity,
        wait_minutes,
        tier,
        owner,
        tags,
        reopen_count
    } = ticket;
    val tag_score = tags |> sum_tags;
    val score = (
        severity,
        wait_minutes,
        wait_limit,
        tier,
        vip_tier,
        tag_score,
        reopen_count
    ) score_fields;
    val urgent = score >= severity_threshold;
    val owner_id = (owner, fallback_owner) owner_or_default;
    val route = (urgent, id, score) route_ticket;

    SupportCandidate {
        id: id,
        owner: owner_id,
        score: score,
        tag_score: tag_score,
        urgent: urgent,
        route: route
    }
}

fun remember_first_unowned: (
    current: Option<Int32>,
    owner: Option<Int32>,
    ticket_id: Int32
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
                    Some(ticket_id)
                }
            }
        }
    }
}

fun initial_state: (audit_notes: List<Int32>) -> SupportState = {
    SupportState {
        total_score: 0,
        urgent_count: 0,
        stale_count: 0,
        first_unowned: None,
        escalation_ids: [],
        audit_notes: audit_notes
    }
}

fun fold_ticket: (
    state: SupportState,
    ticket: SupportTicket,
    wait_limit: Int32,
    severity_threshold: Int32,
    vip_tier: Int32,
    audit_threshold: Int32
) -> SupportState = {
    val SupportState {
        total_score,
        urgent_count,
        stale_count,
        first_unowned,
        escalation_ids,
        audit_notes
    } = state;
    val SupportTicket {
        id,
        severity,
        wait_minutes,
        tier,
        owner,
        tags,
        reopen_count
    } = ticket;
    val tag_score = tags |> sum_tags;
    val score = (
        severity,
        wait_minutes,
        wait_limit,
        tier,
        vip_tier,
        tag_score,
        reopen_count
    ) score_fields;
    val urgent = score >= severity_threshold;
    val stale = wait_minutes > wait_limit;
    val next_escalation_ids = urgent then {
        (escalation_ids, id) list_append
    } else {
        escalation_ids
    };
    val next_audit_notes = score >= audit_threshold then {
        (audit_notes, id + 600) list_append
    } else {
        audit_notes
    };

    SupportState {
        total_score: total_score + score,
        urgent_count: urgent_count + (urgent, 1) points_when,
        stale_count: stale_count + (stale, 1) points_when,
        first_unowned: (first_unowned, owner, id) remember_first_unowned,
        escalation_ids: next_escalation_ids,
        audit_notes: next_audit_notes
    }
}

fun plan_support_escalation: (
    audit_tickets: List<SupportTicket>,
    candidate_tickets: List<SupportTicket>,
    policy: EscalationPolicy,
    seed_notes: List<Int32>
) -> EscalationPlan = {
    val EscalationPolicy {
        wait_limit,
        severity_threshold,
        vip_tier,
        fallback_owner,
        audit_threshold
    } = policy;
    val moved_seed_notes = seed_notes |> move_audit_notes;
    val moved_candidates = candidate_tickets |> move_ticket_stream;
    val normalized_candidates = (moved_candidates, |ticket| ticket |> move_ticket) map;
    val selected_tickets = (
        normalized_candidates,
        |ticket| (ticket, wait_limit, severity_threshold, vip_tier) should_escalate
    ) filter;
    val candidates = (
        selected_tickets,
        |ticket| (ticket, wait_limit, severity_threshold, vip_tier, fallback_owner) candidate_for
    ) map;
    val initial = moved_seed_notes |> initial_state;
    val state = (
        audit_tickets,
        initial,
        |current, ticket| (current, ticket, wait_limit, severity_threshold, vip_tier, audit_threshold) fold_ticket
    ) fold;
    val SupportState {
        total_score,
        urgent_count,
        stale_count,
        first_unowned,
        escalation_ids,
        audit_notes
    } = state;
    val route = (urgent_count, total_score) route_plan;

    EscalationPlan {
        total_score: total_score,
        urgent_count: urgent_count,
        stale_count: stale_count,
        first_unowned: first_unowned,
        candidates: candidates,
        escalation_ids: escalation_ids,
        audit_notes: audit_notes,
        deferred_owner: None,
        route: route
    }
}

fun main: () -> EscalationPlan = {
    val audit_tickets: List<SupportTicket> = [
        SupportTicket {
            id: 701,
            severity: 4,
            wait_minutes: 90,
            tier: 1,
            owner: Some(42),
            tags: [5, 9],
            reopen_count: 1
        },
        SupportTicket {
            id: 702,
            severity: 2,
            wait_minutes: 35,
            tier: 3,
            owner: None,
            tags: [2],
            reopen_count: 0
        }
    ];
    val candidate_tickets: List<SupportTicket> = [
        SupportTicket {
            id: 801,
            severity: 5,
            wait_minutes: 120,
            tier: 1,
            owner: Some(8),
            tags: [10, 4],
            reopen_count: 2
        },
        SupportTicket {
            id: 802,
            severity: 1,
            wait_minutes: 20,
            tier: 2,
            owner: None,
            tags: [1],
            reopen_count: 0
        }
    ];
    val policy = EscalationPolicy {
        wait_limit: 60,
        severity_threshold: 80,
        vip_tier: 1,
        fallback_owner: 900,
        audit_threshold: 70
    };
    val seed_notes = [500];

    (audit_tickets, candidate_tickets, policy, seed_notes) plan_support_escalation
}
