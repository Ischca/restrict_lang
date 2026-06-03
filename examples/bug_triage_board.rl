// Bug triage board example.
// A small issue-prioritization pipeline that dogfoods List<Record> folds,
// Option routing, empty collection inference, lambda expected types, and
// branch-sensitive affine use of lists.

record Ticket {
    id: Int32,
    severity: Int32,
    age_hours: Int32,
    customer_impact: Boolean,
    owner: Option<Int32>,
    signals: List<Int32>
}

record BoardState {
    score_total: Int32,
    page_count: Int32,
    first_unowned: Option<Int32>,
    escalation_codes: List<Int32>,
    scores: List<Int32>
}

record BoardReport {
    score_total: Int32,
    page_count: Int32,
    first_unowned: Option<Int32>,
    escalation_codes: List<Int32>,
    score_cards: List<Int32>,
    accepted: Boolean
}

fun bool_points: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun age_points: (age_hours: Int32) -> Int32 = {
    age_hours > 72 then {
        30
    } else {
        age_hours > 24 then {
            15
        } else {
            0
        }
    }
}

fun signal_points: (signals: List<Int32>) -> Int32 = {
    (signals, 0, |total, signal| total + signal) fold
}

fun first_missing_owner: (
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

fun ticket_score: (
    severity: Int32,
    age_hours: Int32,
    customer_impact: Boolean,
    signals: List<Int32>
) -> Int32 = {
    val severity_points = severity * 10;
    val impact_points = (customer_impact, 25) bool_points;
    val stale_points = age_hours |> age_points;
    val telemetry_points = signals |> signal_points;

    severity_points + impact_points + stale_points + telemetry_points
}

fun add_ticket: (state: BoardState, ticket: Ticket) -> BoardState = {
    val BoardState {
        score_total,
        page_count,
        first_unowned,
        escalation_codes,
        scores
    } = state;
    val Ticket {
        id,
        severity,
        age_hours,
        customer_impact,
        owner,
        signals
    } = ticket;

    val score = (severity, age_hours, customer_impact, signals) ticket_score;
    val should_page = score >= 80;
    val page_delta = (should_page, 1) bool_points;
    val next_unowned = (first_unowned, owner, id) first_missing_owner;
    val next_escalations = should_page then {
        (escalation_codes, id + 900) list_append
    } else {
        escalation_codes
    };
    val next_scores = (scores, score) list_append;

    BoardState {
        score_total: score_total + score,
        page_count: page_count + page_delta,
        first_unowned: next_unowned,
        escalation_codes: next_escalations,
        scores: next_scores
    }
}

fun build_board: (tickets: List<Ticket>) -> BoardReport = {
    val initial = BoardState {
        score_total: 0,
        page_count: 0,
        first_unowned: None,
        escalation_codes: [],
        scores: []
    };
    val state = (tickets, initial, |current, ticket| (current, ticket) add_ticket) fold;
    val BoardState {
        score_total,
        page_count,
        first_unowned,
        escalation_codes,
        scores
    } = state;

    BoardReport {
        score_total: score_total,
        page_count: page_count,
        first_unowned: first_unowned,
        escalation_codes: escalation_codes,
        score_cards: scores,
        accepted: page_count == 0 && score_total < 180
    }
}

fun main: () -> BoardReport = {
    val tickets: List<Ticket> = [
        Ticket {
            id: 101,
            severity: 4,
            age_hours: 80,
            customer_impact: true,
            owner: Some(7),
            signals: [10, 15]
        },
        Ticket {
            id: 102,
            severity: 2,
            age_hours: 18,
            customer_impact: false,
            owner: None,
            signals: [5]
        }
    ];

    tickets |> build_board
}
