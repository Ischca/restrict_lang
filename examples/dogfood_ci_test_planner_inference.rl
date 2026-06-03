// CI test-selection planner dogfood for current non-TAT inference.
// Encodes suites and statuses as Int32 while exercising generic records,
// generic helpers, local generic function values, expected lambdas through
// map/filter/fold, Option/List/Result constructors, and empty literals.

record InferenceBox<T> {
    value: T,
    fallback: Option<T>,
    history: List<T>
}

record CiChange {
    id: Int32,
    suite: Int32,
    risk: Int32,
    touched_files: Int32,
    owner: Option<Int32>,
    status: Int32
}

record CiPolicy {
    min_score: Int32,
    fallback_owner: Int32,
    flaky_status: Int32
}

record CiCandidate {
    id: Int32,
    suite: Int32,
    owner: Int32,
    score: Int32,
    selected: Boolean,
    route: Result<Int32, Int32>
}

record CiPlannerState {
    total_score: Int32,
    selected_count: Int32,
    flaky_count: Int32,
    first_unowned: Option<Int32>,
    selected_ids: List<Int32>,
    quarantined_suites: List<Int32>
}

record CiPlan {
    total_score: Int32,
    selected_count: Int32,
    flaky_count: Int32,
    first_unowned: Option<Int32>,
    candidates: List<CiCandidate>,
    selected_ids: List<Int32>,
    quarantined_suites: List<Int32>,
    skipped_suites: Option<List<Int32>>,
    route: Result<Int32, Int32>
}

fun keep: <T>(value: T) -> T = {
    value
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

fun box_value: <T>(box: InferenceBox<T>) -> T = {
    val InferenceBox {
        value,
        fallback,
        history
    } = box;

    (fallback, value) choose_value
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

fun selection_score: (
    risk: Int32,
    touched_files: Int32,
    suite: Int32,
    status: Int32,
    suite_bias: Int32
) -> Int32 = {
    risk * 3 + touched_files + suite_bias + suite - status
}

fun owner_or_default: (owner: Option<Int32>, fallback_owner: Int32) -> Int32 = {
    val positive_owner = (owner, |value| value > 0) filter;
    val routed_owner = (positive_owner, |value| value + 0) map;

    (routed_owner, fallback_owner) choose_value
}

fun route_candidate: (
    selected: Boolean,
    change_id: Int32,
    suite: Int32
) -> Result<Int32, Int32> = {
    selected then {
        Ok(change_id)
    } else {
        Err(suite)
    }
}

fun route_plan: (selected_count: Int32, total_score: Int32) -> Result<Int32, Int32> = {
    selected_count > 0 then {
        Ok(total_score)
    } else {
        Err(total_score)
    }
}

fun remember_unowned: (
    current: Option<Int32>,
    owner: Option<Int32>,
    change_id: Int32
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
                    Some(change_id)
                }
            }
        }
    }
}

fun should_select_change: (
    change: CiChange,
    min_score: Int32,
    suite_bias: Int32
) -> Boolean = {
    val CiChange {
        id,
        suite,
        risk,
        touched_files,
        owner,
        status
    } = change;
    val score = (risk, touched_files, suite, status, suite_bias) selection_score;

    score >= min_score
}

fun candidate_for: (
    change: CiChange,
    min_score: Int32,
    suite_bias: Int32,
    fallback_owner: Int32
) -> CiCandidate = {
    val CiChange {
        id,
        suite,
        risk,
        touched_files,
        owner,
        status
    } = change;
    val score = (risk, touched_files, suite, status, suite_bias) selection_score;
    val selected = score >= min_score;
    val owner_id = (owner, fallback_owner) owner_or_default;
    val route = (selected, id, suite) route_candidate;

    CiCandidate {
        id: id,
        suite: suite,
        owner: owner_id,
        score: score,
        selected: selected,
        route: route
    }
}

fun initial_state: () -> CiPlannerState = {
    CiPlannerState {
        total_score: 0,
        selected_count: 0,
        flaky_count: 0,
        first_unowned: None,
        selected_ids: [],
        quarantined_suites: []
    }
}

fun blank_plan: () -> CiPlan = {
    CiPlan {
        total_score: 0,
        selected_count: 0,
        flaky_count: 0,
        first_unowned: None,
        candidates: [],
        selected_ids: [],
        quarantined_suites: [],
        skipped_suites: Some([]),
        route: Ok(0)
    }
}

fun fold_change: (
    state: CiPlannerState,
    change: CiChange,
    min_score: Int32,
    suite_bias: Int32,
    flaky_status: Int32
) -> CiPlannerState = {
    val CiPlannerState {
        total_score,
        selected_count,
        flaky_count,
        first_unowned,
        selected_ids,
        quarantined_suites
    } = state;
    val CiChange {
        id,
        suite,
        risk,
        touched_files,
        owner,
        status
    } = change;
    val score = (risk, touched_files, suite, status, suite_bias) selection_score;
    val selected = score >= min_score;
    val flaky = status == flaky_status;
    val next_selected_ids = selected then {
        (selected_ids, id) list_append
    } else {
        selected_ids
    };
    val next_quarantined_suites = flaky then {
        (quarantined_suites, suite) list_append
    } else {
        quarantined_suites
    };

    CiPlannerState {
        total_score: total_score + score,
        selected_count: selected_count + (selected, 1) points_when,
        flaky_count: flaky_count + (flaky, 1) points_when,
        first_unowned: (first_unowned, owner, id) remember_unowned,
        selected_ids: next_selected_ids,
        quarantined_suites: next_quarantined_suites
    }
}

fun plan_ci_tests: (
    audit_changes: List<CiChange>,
    candidate_changes: List<CiChange>,
    suite_bias: InferenceBox<Int32>,
    policy: CiPolicy
) -> CiPlan = {
    val CiPolicy {
        min_score,
        fallback_owner,
        flaky_status
    } = policy;
    val suite_bias_value = suite_bias |> box_value;
    val passthrough = keep;
    val normalized_candidates = (candidate_changes, passthrough) map;
    val selected_changes = (
        normalized_candidates,
        |change| (change, min_score, suite_bias_value) should_select_change
    ) filter;
    val candidates = (
        selected_changes,
        |change| (change, min_score, suite_bias_value, fallback_owner) candidate_for
    ) map;
    val initial = () initial_state;
    val state = (
        audit_changes,
        initial,
        |current, change| (current, change, min_score, suite_bias_value, flaky_status) fold_change
    ) fold;
    val CiPlannerState {
        total_score,
        selected_count,
        flaky_count,
        first_unowned,
        selected_ids,
        quarantined_suites
    } = state;
    val route = (selected_count, total_score) route_plan;

    CiPlan {
        total_score: total_score,
        selected_count: selected_count,
        flaky_count: flaky_count,
        first_unowned: first_unowned,
        candidates: candidates,
        selected_ids: selected_ids,
        quarantined_suites: quarantined_suites,
        skipped_suites: None,
        route: route
    }
}

fun main: () -> CiPlan = {
    val audit_changes: List<CiChange> = [
        CiChange {
            id: 101,
            suite: 1,
            risk: 8,
            touched_files: 3,
            owner: Some(42),
            status: 0
        },
        CiChange {
            id: 102,
            suite: 2,
            risk: 4,
            touched_files: 8,
            owner: None,
            status: 2
        }
    ];
    val candidate_changes: List<CiChange> = [
        CiChange {
            id: 201,
            suite: 1,
            risk: 7,
            touched_files: 6,
            owner: Some(7),
            status: 0
        },
        CiChange {
            id: 202,
            suite: 3,
            risk: 2,
            touched_files: 2,
            owner: None,
            status: 2
        }
    ];
    val suite_bias = InferenceBox {
        value: 4,
        fallback: None,
        history: []
    };
    val policy = CiPolicy {
        min_score: 20,
        fallback_owner: 900,
        flaky_status: 2
    };

    (audit_changes, candidate_changes, suite_bias, policy) plan_ci_tests
}
