// Sprint planner example.
// A small planning pipeline that dogfoods generic type inference:
// lambda-first calls, map/filter/fold, empty list inference, and None inference.

record Task {
    id: Int32,
    effort: Int32,
    impact: Int32,
    blocked: Boolean,
    owner: Option<Int32>
}

record PlanningState {
    ready_effort: Int32,
    ready_impact: Int32,
    blocked_count: Int32,
    first_unowned: Option<Int32>
}

record SprintPlan {
    score: Int32,
    ready_effort: Int32,
    blocked_count: Int32,
    unowned_task: Option<Int32>,
    escalation_codes: List<Int32>,
    candidate_scores: List<Int32>
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun bool_points: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun is_ready: (task: Task) -> Boolean = {
    val Task { id, effort, impact, blocked, owner } = task;
    blocked match {
        true => { false }
        false => { true }
    }
}

fun score_task: (task: Task) -> Int32 = {
    val Task { id, effort, impact, blocked, owner } = task;
    val blocked_penalty = (blocked, 30) bool_points;
    impact * 10 - effort - blocked_penalty
}

fun score_one: (task: Task) -> Int32 = {
    (|candidate| candidate |> score_task, task) apply_first
}

fun score_candidates: (tasks: List<Task>) -> List<Int32> = {
    val ready = (tasks, |task| task |> is_ready) filter;
    (ready, |task| task |> score_one) map
}

fun first_unowned_task: (
    current: Option<Int32>,
    owner: Option<Int32>,
    task_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            owner match {
                Some(person) => { None }
                None => { Some(task_id) }
            }
        }
    }
}

fun add_task: (state: PlanningState, task: Task) -> PlanningState = {
    val PlanningState { ready_effort, ready_impact, blocked_count, first_unowned } = state;
    val Task { id, effort, impact, blocked, owner } = task;
    val ready_effort_delta = blocked match {
        true => { 0 }
        false => { effort }
    };
    val ready_impact_delta = blocked match {
        true => { 0 }
        false => { impact }
    };

    PlanningState {
        ready_effort: ready_effort + ready_effort_delta,
        ready_impact: ready_impact + ready_impact_delta,
        blocked_count: blocked_count + (blocked, 1) bool_points,
        first_unowned: (first_unowned, owner, id) first_unowned_task
    }
}

fun build_plan: (
    tasks: List<Task>,
    candidate_scores: List<Int32>,
    fallback_codes: List<Int32>
) -> SprintPlan = {
    val initial = PlanningState {
        ready_effort: 0,
        ready_impact: 0,
        blocked_count: 0,
        first_unowned: None
    };
    val state = (tasks, initial, |current, task| (current, task) add_task) fold;
    val PlanningState { ready_effort, ready_impact, blocked_count, first_unowned } = state;
    val escalation_codes = ([], fallback_codes) choose_first;
    val unowned_task = (None, first_unowned) choose_first;

    SprintPlan {
        score: ready_impact * 10 - ready_effort,
        ready_effort: ready_effort,
        blocked_count: blocked_count,
        unowned_task: unowned_task,
        escalation_codes: escalation_codes,
        candidate_scores: candidate_scores
    }
}

fun main: () -> SprintPlan = {
    val scoring_tasks: List<Task> = [
        Task {
            id: 1,
            effort: 3,
            impact: 8,
            blocked: false,
            owner: Some(42)
        },
        Task {
            id: 2,
            effort: 5,
            impact: 13,
            blocked: false,
            owner: None
        }
    ];
    val candidate_scores = scoring_tasks |> score_candidates;

    val planning_tasks: List<Task> = [
        Task {
            id: 1,
            effort: 3,
            impact: 8,
            blocked: false,
            owner: Some(42)
        },
        Task {
            id: 3,
            effort: 8,
            impact: 21,
            blocked: true,
            owner: None
        }
    ];
    val fallback_codes = [700, 701];

    (planning_tasks, candidate_scores, fallback_codes) build_plan
}
