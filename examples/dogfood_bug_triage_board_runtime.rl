// Dogfood bug triage board runtime example.
// Keeps the board report as an internal record while exposing a scalar
// host-callable wrapper for v0.0.1 WebAssembly runtime execution.

record RuntimeTicket {
    id: Int32,
    severity: Int32,
    age_hours: Int32,
    customer_impact: Boolean,
    owner: Option<Int32>,
    signals: List<Int32>
}

record RuntimeBoardState {
    score_total: Int32,
    page_count: Int32,
    first_unowned: Option<Int32>,
    escalation_codes: List<Int32>,
    scores: List<Int32>
}

record RuntimeBoardReport {
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

fun add_ticket: (state: RuntimeBoardState, ticket: RuntimeTicket) -> RuntimeBoardState = {
    val RuntimeBoardState {
        score_total,
        page_count,
        first_unowned,
        escalation_codes,
        scores
    } = state;
    val RuntimeTicket {
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

    RuntimeBoardState {
        score_total: score_total + score,
        page_count: page_count + page_delta,
        first_unowned: next_unowned,
        escalation_codes: next_escalations,
        scores: next_scores
    }
}

fun build_board: (tickets: List<RuntimeTicket>) -> RuntimeBoardReport = {
    val initial = RuntimeBoardState {
        score_total: 0,
        page_count: 0,
        first_unowned: None,
        escalation_codes: [],
        scores: []
    };
    val state = (tickets, initial, |current, ticket| (current, ticket) add_ticket) fold;
    val RuntimeBoardState {
        score_total,
        page_count,
        first_unowned,
        escalation_codes,
        scores
    } = state;

    RuntimeBoardReport {
        score_total: score_total,
        page_count: page_count,
        first_unowned: first_unowned,
        escalation_codes: escalation_codes,
        score_cards: scores,
        accepted: page_count == 0 && score_total < 180
    }
}

fun sample_bug_triage_board: () -> RuntimeBoardReport = {
    val first_signals: List<Int32> = [10, 15];
    val first_ticket = RuntimeTicket {
        id: 101,
        severity: 4,
        age_hours: 80,
        customer_impact: true,
        owner: Some(7),
        signals: first_signals
    };
    val second_signals: List<Int32> = [5];
    val second_ticket = RuntimeTicket {
        id: 102,
        severity: 2,
        age_hours: 18,
        customer_impact: false,
        owner: None,
        signals: second_signals
    };
    val tickets: List<RuntimeTicket> = [
        first_ticket,
        second_ticket
    ];

    tickets |> build_board
}

export fun bug_triage_board_runtime_score: () -> Int32 = {
    val report = () sample_bug_triage_board;
    val RuntimeBoardReport {
        score_total,
        page_count,
        first_unowned,
        escalation_codes,
        score_cards,
        accepted
    } = report;
    val owner_score = first_unowned match {
        Some(ticket_id) => { ticket_id }
        None => { 0 }
    };
    val escalation_count = escalation_codes |> list_count;
    val score_card_count = score_cards |> list_count;
    val accepted_score = accepted then {
        1000
    } else {
        0
    };

    score_total + page_count + owner_score + escalation_count + score_card_count + accepted_score
}
