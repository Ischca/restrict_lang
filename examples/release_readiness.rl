// Release readiness example.
// Dogfoods a practical release-gate workflow with records, Option flow,
// map/filter/fold, empty collection inference, None inference, immediate
// lambda pipes, and higher-order generic calls.

record Change {
    id: Int32,
    risk: Int32,
    test_coverage: Int32,
    customer_visible: Boolean,
    owner: Option<Int32>
}

record ReleaseState {
    total_risk: Int32,
    uncovered_count: Int32,
    customer_visible_count: Int32,
    first_unowned: Option<Int32>
}

record ReleaseDecision {
    approved: Boolean,
    risk_score: Int32,
    uncovered_count: Int32,
    missing_owner: Option<Int32>,
    blocker_codes: List<Int32>,
    review_scores: List<Int32>
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun coverage_gap: (coverage: Int32) -> Int32 = {
    coverage < 80 then {
        1
    } else {
        0
    }
}

fun change_risk: (change: Change) -> Int32 = {
    val Change { id, risk, test_coverage, customer_visible, owner } = change;
    val coverage_penalty = (test_coverage < 80, 25) points_when;
    val visibility_penalty = (customer_visible, 10) points_when;
    val base = risk + coverage_penalty;
    base |> (|score| score + visibility_penalty)
}

fun needs_review: (change: Change) -> Boolean = {
    val Change { id, risk, test_coverage, customer_visible, owner } = change;
    val high_risk = risk > 40;
    high_risk match {
        true => { true }
        false => { test_coverage < 80 }
    }
}

fun review_score: (change: Change) -> Int32 = {
    (|candidate| candidate |> change_risk, change) apply_first
}

fun review_scores: (changes: List<Change>) -> List<Int32> = {
    val review_changes = (changes, |change| change |> needs_review) filter;
    val raw_scores = (review_changes, |change| change |> review_score) map;
    (raw_scores, |score| score |> (|value| value + 1)) map
}

fun first_missing_owner: (
    current: Option<Int32>,
    owner: Option<Int32>,
    change_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(change_id) }
            }
        }
    }
}

fun accumulate_change: (state: ReleaseState, change: Change) -> ReleaseState = {
    val ReleaseState {
        total_risk,
        uncovered_count,
        customer_visible_count,
        first_unowned
    } = state;
    val Change { id, risk, test_coverage, customer_visible, owner } = change;
    val coverage_penalty = (test_coverage < 80, 25) points_when;
    val visibility_penalty = (customer_visible, 10) points_when;
    val change_risk_value = (risk + coverage_penalty) |> (|score| score + visibility_penalty);

    ReleaseState {
        total_risk: total_risk + change_risk_value,
        uncovered_count: uncovered_count + (test_coverage |> coverage_gap),
        customer_visible_count: customer_visible_count + (customer_visible, 1) points_when,
        first_unowned: (first_unowned, owner, id) first_missing_owner
    }
}

fun finish_decision: (
    state: ReleaseState,
    review_scores_value: List<Int32>,
    fallback_codes: List<Int32>
) -> ReleaseDecision = {
    val ReleaseState {
        total_risk,
        uncovered_count,
        customer_visible_count,
        first_unowned
    } = state;
    val blocker_codes: List<Int32> = [] |> (|empty_codes| (empty_codes, fallback_codes) choose_first);
    val missing_owner: Option<Int32> = None |> (|empty_owner| (empty_owner, first_unowned) choose_first);
    val approval_threshold = customer_visible_count |> (|count| count * 15 + 80);

    ReleaseDecision {
        approved: total_risk < approval_threshold,
        risk_score: total_risk,
        uncovered_count: uncovered_count,
        missing_owner: missing_owner,
        blocker_codes: blocker_codes,
        review_scores: review_scores_value
    }
}

fun assess_release: (
    changes: List<Change>,
    scores_input: List<Change>,
    fallback_codes: List<Int32>
) -> ReleaseDecision = {
    val initial = ReleaseState {
        total_risk: 0,
        uncovered_count: 0,
        customer_visible_count: 0,
        first_unowned: None
    };
    val final_state = (changes, initial, |state, change| (state, change) accumulate_change) fold;
    val scores = scores_input |> review_scores;
    (final_state, scores, fallback_codes) finish_decision
}

fun main: () -> ReleaseDecision = {
    val aggregate_changes: List<Change> = [
        Change {
            id: 101,
            risk: 22,
            test_coverage: 91,
            customer_visible: true,
            owner: Some(7)
        },
        Change {
            id: 102,
            risk: 45,
            test_coverage: 73,
            customer_visible: false,
            owner: None
        }
    ];
    val scoring_changes: List<Change> = [
        Change {
            id: 201,
            risk: 55,
            test_coverage: 85,
            customer_visible: true,
            owner: Some(9)
        },
        Change {
            id: 202,
            risk: 12,
            test_coverage: 72,
            customer_visible: false,
            owner: Some(10)
        }
    ];
    val fallback_codes = [900, 901];

    (aggregate_changes, scoring_changes, fallback_codes) assess_release
}
