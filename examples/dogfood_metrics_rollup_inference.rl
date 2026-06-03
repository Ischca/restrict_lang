// Dogfood metrics rollup for current non-TAT inference.
// Exercises generic records, Option/Result/List constructors, OSV calls,
// function values, lambdas, filter/map/fold, and expected-type contexts
// for empty collections.

record MetricSlot<T> {
    value: T,
    fallback: Option<T>
}

record MetricSample {
    key: Int32,
    current: Int32,
    previous: Option<Int32>,
    weight: Int32
}

record MetricPolicy {
    warning_limit: Int32,
    critical_limit: Int32,
    owner_hint: MetricSlot<Int32>
}

record MetricScore {
    key: Int32,
    current: Int32,
    delta: Int32,
    weighted: Int32,
    healthy: Boolean,
    route: Result<Int32, Int32>
}

record MetricRollupState {
    total_weighted: Int32,
    warning_count: Int32,
    critical_count: Int32,
    first_missing_previous: Option<Int32>,
    warning_keys: List<Int32>
}

record MetricReport {
    total_weighted: Int32,
    warning_count: Int32,
    critical_count: Int32,
    first_missing_previous: Option<Int32>,
    scored: List<MetricScore>,
    warning_keys: List<Int32>,
    sampled_keys: Option<List<Int32>>
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

fun slot_choose: <T>(slot: MetricSlot<T>) -> T = {
    val MetricSlot {
        value,
        fallback
    } = slot;

    (fallback, value) choose_value
}

fun sample_has_previous: (sample: MetricSample) -> Boolean = {
    val MetricSample {
        key,
        current,
        previous,
        weight
    } = sample;

    previous match {
        Some(value) => {
            true
        }
        None => {
            false
        }
    }
}

fun route_sample: (
    healthy: Boolean,
    key: Int32,
    fallback_owner: Int32
) -> Result<Int32, Int32> = {
    healthy then {
        Ok(key)
    } else {
        Err(fallback_owner)
    }
}

fun delta_from_previous: (current: Int32, previous: Option<Int32>) -> Int32 = {
    val baseline = (previous, current) choose_value;

    current - baseline
}

fun score_sample: (
    sample: MetricSample,
    warning_limit: Int32,
    critical_limit: Int32,
    fallback_owner: Int32
) -> MetricScore = {
    val MetricSample {
        key,
        current,
        previous,
        weight
    } = sample;
    val delta = (current, previous) delta_from_previous;
    val overloaded = current >= critical_limit;
    val healthy = overloaded match {
        true => {
            false
        }
        false => {
            true
        }
    };
    val route = (healthy, key, fallback_owner) route_sample;

    MetricScore {
        key: key,
        current: current,
        delta: delta,
        weighted: current * weight + delta,
        healthy: healthy,
        route: route
    }
}

fun remember_missing_previous: (
    current_missing: Option<Int32>,
    previous: Option<Int32>,
    key: Int32
) -> Option<Int32> = {
    current_missing match {
        Some(existing) => {
            Some(existing)
        }
        None => {
            previous match {
                Some(value) => {
                    None
                }
                None => {
                    Some(key)
                }
            }
        }
    }
}

fun initial_state: () -> MetricRollupState = {
    MetricRollupState {
        total_weighted: 0,
        warning_count: 0,
        critical_count: 0,
        first_missing_previous: None,
        warning_keys: []
    }
}

fun blank_report: () -> MetricReport = {
    MetricReport {
        total_weighted: 0,
        warning_count: 0,
        critical_count: 0,
        first_missing_previous: None,
        scored: [],
        warning_keys: [],
        sampled_keys: Some([])
    }
}

fun fold_sample: (
    state: MetricRollupState,
    sample: MetricSample,
    warning_limit: Int32,
    critical_limit: Int32
) -> MetricRollupState = {
    val MetricRollupState {
        total_weighted,
        warning_count,
        critical_count,
        first_missing_previous,
        warning_keys
    } = state;
    val MetricSample {
        key,
        current,
        previous,
        weight
    } = sample;
    val warned = current >= warning_limit;
    val critical = current >= critical_limit;
    val delta = (current, previous) delta_from_previous;
    val weighted = current * weight + delta;
    val next_warning_keys = warned then {
        (warning_keys, key) list_append
    } else {
        warning_keys
    };

    MetricRollupState {
        total_weighted: total_weighted + weighted,
        warning_count: warning_count + (warned, 1) points_when,
        critical_count: critical_count + (critical, 1) points_when,
        first_missing_previous: (first_missing_previous, previous, key) remember_missing_previous,
        warning_keys: next_warning_keys
    }
}

fun rollup_metrics: (
    audit_samples: List<MetricSample>,
    score_samples: List<MetricSample>,
    policy: MetricPolicy
) -> MetricReport = {
    val MetricPolicy {
        warning_limit,
        critical_limit,
        owner_hint
    } = policy;
    val fallback_owner = owner_hint |> slot_choose;
    val usable_samples = (score_samples, sample_has_previous) filter;
    val scored = (
        usable_samples,
        |sample| (sample, warning_limit, critical_limit, fallback_owner) score_sample
    ) map;
    val initial = () initial_state;
    val state = (
        audit_samples,
        initial,
        |current, sample| (current, sample, warning_limit, critical_limit) fold_sample
    ) fold;
    val MetricRollupState {
        total_weighted,
        warning_count,
        critical_count,
        first_missing_previous,
        warning_keys
    } = state;

    MetricReport {
        total_weighted: total_weighted,
        warning_count: warning_count,
        critical_count: critical_count,
        first_missing_previous: first_missing_previous,
        scored: scored,
        warning_keys: warning_keys,
        sampled_keys: None
    }
}

fun main: () -> MetricReport = {
    val audit_samples: List<MetricSample> = [
        MetricSample {
            key: 11,
            current: 72,
            previous: Some(70),
            weight: 2
        },
        MetricSample {
            key: 12,
            current: 96,
            previous: None,
            weight: 3
        }
    ];
    val score_samples: List<MetricSample> = [
        MetricSample {
            key: 21,
            current: 81,
            previous: Some(80),
            weight: 1
        },
        MetricSample {
            key: 22,
            current: 41,
            previous: None,
            weight: 1
        }
    ];
    val policy = MetricPolicy {
        warning_limit: 75,
        critical_limit: 90,
        owner_hint: MetricSlot {
            value: 404,
            fallback: None
        }
    };

    (audit_samples, score_samples, policy) rollup_metrics
}
