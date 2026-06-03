// Support queue example.
// A small ticket prioritization pipeline that exercises records, Option,
// then expressions, fold accumulators, and expected-type lambda inference.

record Ticket {
    id: Int32,
    severity: Int32,
    age_hours: Int32,
    vip: Boolean,
    owner: Option<Int32>
}

record QueueState {
    total_score: Int32,
    overdue_count: Int32,
    first_unowned: Option<Int32>,
    routed_count: Int32
}

record QueueReport {
    total_score: Int32,
    overdue_count: Int32,
    first_unowned: Option<Int32>,
    routed_count: Int32,
    routed_scores: List<Int32>,
    escalation_codes: List<Int32>
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun owner_missing: (owner: Option<Int32>) -> Boolean = {
    owner match {
        Some(person) => { false }
        None => { true }
    }
}

fun overdue_points: (age_hours: Int32) -> Int32 = {
    age_hours > 48 then {
        10
    } else {
        0
    }
}

fun should_route: (ticket: Ticket) -> Boolean = {
    val Ticket { id, severity, age_hours, vip, owner } = ticket;
    val missing = owner |> owner_missing;
    val high_severity = severity > 2;

    high_severity match {
        true => { true }
        false => { missing }
    }
}

fun ticket_score: (ticket: Ticket) -> Int32 = {
    val Ticket { id, severity, age_hours, vip, owner } = ticket;
    val age_points = age_hours |> overdue_points;
    val vip_points = (vip, 12) points_when;
    val missing = owner |> owner_missing;
    val missing_points = (missing, 3) points_when;

    severity * 10 + age_points + vip_points + missing_points
}

fun routed_scores: (tickets: List<Ticket>) -> List<Int32> = {
    val routed = (tickets, |ticket| ticket |> should_route) filter;
    (routed, |ticket| ticket |> ticket_score) map
}

fun choose_first_unowned: (
    current: Option<Int32>,
    owner: Option<Int32>,
    ticket_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(ticket_id) }
            }
        }
    }
}

fun add_ticket: (state: QueueState, ticket: Ticket) -> QueueState = {
    val QueueState { total_score, overdue_count, first_unowned, routed_count } = state;
    val Ticket { id, severity, age_hours, vip, owner } = ticket;
    val overdue = age_hours > 48;
    val high_severity = severity > 2;
    val missing = owner |> owner_missing;
    val route_delta = high_severity match {
        true => { 1 }
        false => { (missing, 1) points_when }
    };

    QueueState {
        total_score: total_score + severity * 10 + (overdue, 10) points_when,
        overdue_count: overdue_count + (overdue, 1) points_when,
        first_unowned: (first_unowned, owner, id) choose_first_unowned,
        routed_count: routed_count + route_delta
    }
}

fun build_report: (
    tickets: List<Ticket>,
    scores: List<Int32>,
    fallback_codes: List<Int32>
) -> QueueReport = {
    val initial = QueueState {
        total_score: 0,
        overdue_count: 0,
        first_unowned: None,
        routed_count: 0
    };
    val final_state = (tickets, initial, |current, ticket| (current, ticket) add_ticket) fold;
    val QueueState { total_score, overdue_count, first_unowned, routed_count } = final_state;
    val escalation_codes = ([900, 901], fallback_codes) choose_first;

    QueueReport {
        total_score: total_score,
        overdue_count: overdue_count,
        first_unowned: first_unowned,
        routed_count: routed_count,
        routed_scores: scores,
        escalation_codes: escalation_codes
    }
}

fun main: () -> QueueReport = {
    val scoring_tickets: List<Ticket> = [
        Ticket {
            id: 10,
            severity: 4,
            age_hours: 72,
            vip: true,
            owner: Some(7)
        },
        Ticket {
            id: 11,
            severity: 1,
            age_hours: 12,
            vip: false,
            owner: None
        }
    ];
    val scores = scoring_tickets |> routed_scores;

    val report_tickets: List<Ticket> = [
        Ticket {
            id: 10,
            severity: 4,
            age_hours: 72,
            vip: true,
            owner: Some(7)
        },
        Ticket {
            id: 12,
            severity: 2,
            age_hours: 60,
            vip: false,
            owner: None
        }
    ];
    val fallback_codes = [800];

    (report_tickets, scores, fallback_codes) build_report
}
