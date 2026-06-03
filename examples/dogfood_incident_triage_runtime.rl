// Dogfood incident triage runtime example.
// Keeps the report as an internal record while exposing a scalar host-callable
// wrapper for v0.0.1 WebAssembly runtime execution.

record Alert {
    id: Int32,
    severity: Int32,
    owner: Option<Int32>,
    stale: Boolean,
    acknowledged: Boolean
}

record TriagePolicy {
    page_threshold: Int32,
    fallback_owner: Int32
}

record TriageState {
    total_score: Int32,
    stale_count: Int32,
    page_count: Int32,
    first_unowned: Option<Int32>,
    routed_codes: List<Int32>
}

record TriageReport {
    risk_score: Int32,
    stale_count: Int32,
    owner_gap: Option<Int32>,
    page_count: Int32,
    page_codes: List<Int32>,
    routed_codes: List<Int32>
}

fun choose_owner: (owner: Option<Int32>, fallback_owner: Int32) -> Int32 = {
    owner match {
        Some(person) => { person }
        None => { fallback_owner }
    }
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun page_delta: (
    severity: Int32,
    acknowledged: Boolean,
    page_threshold: Int32
) -> Int32 = {
    val severity_page = severity >= page_threshold then {
        1
    } else {
        0
    };
    val ack_page = acknowledged match {
        true => { 0 }
        false => { 1 }
    };

    severity_page * ack_page
}

fun first_missing_owner: (
    current: Option<Int32>,
    owner: Option<Int32>,
    alert_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(alert_id) }
            }
        }
    }
}

fun should_page: (alert: Alert, page_threshold: Int32) -> Boolean = {
    val Alert {
        id,
        severity,
        owner,
        stale,
        acknowledged
    } = alert;
    val urgent = severity >= page_threshold;

    acknowledged match {
        true => { false }
        false => { urgent }
    }
}

fun page_code: (alert: Alert, fallback_owner: Int32) -> Int32 = {
    val Alert {
        id,
        severity,
        owner,
        stale,
        acknowledged
    } = alert;
    val owner_id = (owner, fallback_owner) choose_owner;

    id + severity * 10 + owner_id
}

fun add_alert: (
    state: TriageState,
    alert: Alert,
    page_threshold: Int32
) -> TriageState = {
    val TriageState {
        total_score,
        stale_count,
        page_count,
        first_unowned,
        routed_codes
    } = state;
    val Alert {
        id,
        severity,
        owner,
        stale,
        acknowledged
    } = alert;
    val stale_points = (stale, 5) points_when;
    val ack_points = (acknowledged, 2) points_when;
    val page_points = ((severity, acknowledged, page_threshold) page_delta) * 20;
    val alert_score = severity * 10 + stale_points + ack_points + page_points;
    val next_page_count = page_count + (severity, acknowledged, page_threshold) page_delta;
    val next_routed_codes = stale then {
        (routed_codes, id + 900) list_append
    } else {
        routed_codes
    };

    TriageState {
        total_score: total_score + alert_score,
        stale_count: stale_count + (stale, 1) points_when,
        page_count: next_page_count,
        first_unowned: (first_unowned, owner, id) first_missing_owner,
        routed_codes: next_routed_codes
    }
}

fun initial_state: () -> TriageState = {
    TriageState {
        total_score: 0,
        stale_count: 0,
        page_count: 0,
        first_unowned: None,
        routed_codes: []
    }
}

fun finish_report: (
    state: TriageState,
    page_codes: List<Int32>
) -> TriageReport = {
    val TriageState {
        total_score,
        stale_count,
        page_count,
        first_unowned,
        routed_codes
    } = state;

    TriageReport {
        risk_score: total_score,
        stale_count: stale_count,
        owner_gap: first_unowned,
        page_count: page_count,
        page_codes: page_codes,
        routed_codes: routed_codes
    }
}

fun triage_alerts: (
    audit_alerts: List<Alert>,
    paging_alerts: List<Alert>,
    policy: TriagePolicy
) -> TriageReport = {
    val TriagePolicy {
        page_threshold,
        fallback_owner
    } = policy;
    val actionable_alerts = (
        paging_alerts,
        |alert| (alert, page_threshold) should_page
    ) filter;
    val page_codes = (
        actionable_alerts,
        |alert| (alert, fallback_owner) page_code
    ) map;
    val initial = () initial_state;
    val state = (
        audit_alerts,
        initial,
        |current, alert| (current, alert, page_threshold) add_alert
    ) fold;

    (state, page_codes) finish_report
}

fun sample_incident_report: () -> TriageReport = {
    val audit_alerts: List<Alert> = [
        Alert {
            id: 11,
            severity: 5,
            owner: Some(17),
            stale: true,
            acknowledged: false
        },
        Alert {
            id: 12,
            severity: 2,
            owner: None,
            stale: false,
            acknowledged: true
        }
    ];
    val paging_alerts: List<Alert> = [
        Alert {
            id: 21,
            severity: 4,
            owner: None,
            stale: false,
            acknowledged: false
        },
        Alert {
            id: 22,
            severity: 3,
            owner: Some(9),
            stale: true,
            acknowledged: false
        },
        Alert {
            id: 23,
            severity: 5,
            owner: Some(8),
            stale: false,
            acknowledged: true
        }
    ];
    val policy = TriagePolicy {
        page_threshold: 4,
        fallback_owner: 7
    };

    (audit_alerts, paging_alerts, policy) triage_alerts
}

export fun incident_triage_runtime_score: () -> Int32 = {
    val report = () sample_incident_report;
    val TriageReport {
        risk_score,
        stale_count,
        owner_gap,
        page_count,
        page_codes,
        routed_codes
    } = report;
    val owner_score = owner_gap match {
        Some(alert_id) => { alert_id }
        None => { 0 }
    };
    val page_code_count = page_codes |> list_count;
    val routed_code_count = routed_codes |> list_count;

    risk_score + stale_count + owner_score + page_count + page_code_count + routed_code_count
}
