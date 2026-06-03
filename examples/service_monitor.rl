// Service monitor example.
// Models a small alerting pipeline with records, Option ownership,
// Result-based evidence construction, fold, empty list inference,
// and logical negation.

record ServiceProbe {
    id: Int32,
    latency_ms: Int32,
    failures: Int32,
    stale: Boolean,
    owner: Option<Int32>
}

record ServiceRollup {
    total_latency: Int32,
    failed_count: Int32,
    stale_count: Int32,
    first_unowned: Option<Int32>
}

record ServiceAlert {
    severity: Int32,
    page: Boolean,
    first_unowned: Option<Int32>,
    evidence: List<Int32>,
    message: String
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun remember_unowned: (
    current: Option<Int32>,
    owner: Option<Int32>,
    probe_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(probe_id) }
            }
        }
    }
}

fun add_probe: (state: ServiceRollup, probe: ServiceProbe) -> ServiceRollup = {
    val ServiceRollup {
        total_latency,
        failed_count,
        stale_count,
        first_unowned
    } = state;
    val ServiceProbe { id, latency_ms, failures, stale, owner } = probe;
    val failed = failures > 0 || latency_ms > 800;

    ServiceRollup {
        total_latency: total_latency + latency_ms,
        failed_count: failed_count + (failed, 1) points_when,
        stale_count: stale_count + (stale, 1) points_when,
        first_unowned: (first_unowned, owner, id) remember_unowned
    }
}

fun evidence_codes: (
    failed_count: Int32,
    stale_count: Int32
) -> Result<List<Int32>, Int32> = {
    failed_count < 0 then {
        Err(500)
    } else {
        failed_count > 0 then {
            Ok([100, failed_count])
        } else {
            stale_count > 0 then {
                Ok([200, stale_count])
            } else {
                Ok([])
            }
        }
    }
}

fun severity_from: (failed_count: Int32, stale_count: Int32) -> Int32 = {
    val clean = failed_count == 0 && stale_count == 0;

    !clean then {
        failed_count * 10 + stale_count * 3
    } else {
        0
    }
}

fun alert_message: (page: Boolean) -> String = {
    page then {
        "page: " + "service unhealthy"
    } else {
        "monitor: " + "service stable"
    }
}

fun finish_alert: (rollup: ServiceRollup, quiet: Boolean) -> ServiceAlert = {
    val ServiceRollup {
        total_latency,
        failed_count,
        stale_count,
        first_unowned
    } = rollup;
    val severity = (failed_count, stale_count) severity_from;
    val page = severity >= 10 && !quiet;
    val evidence_result = (failed_count, stale_count) evidence_codes;

    evidence_result match {
        Ok(codes) => {
            ServiceAlert {
                severity: severity,
                page: page,
                first_unowned: first_unowned,
                evidence: codes,
                message: page |> alert_message
            }
        }
        Err(code) => {
            ServiceAlert {
                severity: 99,
                page: true,
                first_unowned: first_unowned,
                evidence: [code],
                message: true |> alert_message
            }
        }
    }
}

fun main: () -> ServiceAlert = {
    val initial = ServiceRollup {
        total_latency: 0,
        failed_count: 0,
        stale_count: 0,
        first_unowned: None
    };
    val probes: List<ServiceProbe> = [
        ServiceProbe {
            id: 10,
            latency_ms: 120,
            failures: 0,
            stale: false,
            owner: Some(7)
        },
        ServiceProbe {
            id: 11,
            latency_ms: 920,
            failures: 1,
            stale: true,
            owner: None
        }
    ];
    val rollup = (probes, initial, |state, probe| (state, probe) add_probe) fold;
    val alert = (rollup, false) finish_alert;
    val ServiceAlert {
        severity,
        page,
        first_unowned,
        evidence,
        message
    } = alert;

    ServiceAlert {
        severity: severity,
        page: page,
        first_unowned: first_unowned,
        evidence: evidence,
        message: "monitor: " + "rollup complete"
    }
}
