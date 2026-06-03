// Incident triage example.
// A small alert scoring pipeline that exercises records, Option flow,
// fold with a record accumulator, and expected-type lambda inference.

record Alert {
    severity: Int32,
    owner: Option<Int32>,
    stale: Boolean,
    acknowledged: Boolean
}

record TriageState {
    total_score: Int32,
    stale_count: Int32,
    first_unowned: Option<Int32>,
    page_count: Int32
}

record TriageReport {
    risk_score: Int32,
    stale_count: Int32,
    owner_gap: Option<Int32>,
    page_count: Int32
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun page_delta: (severity: Int32, acknowledged: Boolean) -> Int32 = {
    val severity_page = severity > 3 then {
        1
    } else {
        0
    };
    val ack_page = acknowledged match {
        true => { 0 }
        false => { 1 }
    };
    severity_page + ack_page
}

fun should_page: (alert: Alert) -> Boolean = {
    val Alert { severity, owner, stale, acknowledged } = alert;
    val urgent = severity > 3;
    acknowledged match {
        true => { false }
        false => { urgent }
    }
}

fun page_severity: (alert: Alert) -> Int32 = {
    val Alert { severity, owner, stale, acknowledged } = alert;
    severity
}

fun paging_severities: (alerts: List<Alert>) -> List<Int32> = {
    val page_alerts = (alerts, |alert| alert |> should_page) filter;
    (page_alerts, |alert| alert |> page_severity) map
}

fun first_missing_owner: (
    current: Option<Int32>,
    owner: Option<Int32>,
    severity: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(severity) }
            }
        }
    }
}

fun add_alert: (state: TriageState, alert: Alert) -> TriageState = {
    val TriageState { total_score, stale_count, first_unowned, page_count } = state;
    val Alert { severity, owner, stale, acknowledged } = alert;
    val stale_points = (stale, 5) points_when;
    val ack_points = (acknowledged, 2) points_when;
    val alert_score = severity * 10 + stale_points + ack_points;
    val next_stale_count = stale_count + (stale, 1) points_when;
    val next_page_count = page_count + (severity, acknowledged) page_delta;

    TriageState {
        total_score: total_score + alert_score,
        stale_count: next_stale_count,
        first_unowned: (first_unowned, owner, severity) first_missing_owner,
        page_count: next_page_count
    }
}

fun finish_report: (state: TriageState) -> TriageReport = {
    val TriageState { total_score, stale_count, first_unowned, page_count } = state;
    TriageReport {
        risk_score: total_score,
        stale_count: stale_count,
        owner_gap: first_unowned,
        page_count: page_count
    }
}

fun triage_alerts: (alerts: List<Alert>) -> TriageReport = {
    val initial = TriageState {
        total_score: 0,
        stale_count: 0,
        first_unowned: None,
        page_count: 0
    };
    val final_state = (alerts, initial, |state, alert| (state, alert) add_alert) fold;
    final_state |> finish_report
}

fun main: () -> TriageReport = {
    val alerts: List<Alert> = [
        Alert {
            severity: 4,
            owner: Some(17),
            stale: true,
            acknowledged: false
        },
        Alert {
            severity: 2,
            owner: None,
            stale: false,
            acknowledged: true
        }
    ];

    alerts |> triage_alerts
}
