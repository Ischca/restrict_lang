// Dogfood task queue inference example.
// Builds a small queue plan while exercising current non-TAT inference:
// generic Option defaults, lambda expected types, list filter/map/fold over
// records, Result construction, and empty List/None fields from record context.

record Task {
    id: Int32,
    priority: Int32,
    estimate: Int32,
    blocked: Boolean,
    owner: Option<Int32>
}

record QueuePolicy {
    max_estimate: Int32,
    alert_threshold: Int32,
    fallback_owner: Int32
}

record TaskCard {
    id: Int32,
    owner: Int32,
    score: Int32,
    ready: Boolean,
    route: Result<Int32, Int32>
}

record QueueState {
    total_score: Int32,
    ready_count: Int32,
    blocked_count: Int32,
    first_unowned: Option<Int32>,
    alert_codes: List<Int32>
}

record QueuePlan {
    total_score: Int32,
    ready_count: Int32,
    blocked_count: Int32,
    first_unowned: Option<Int32>,
    cards: List<TaskCard>,
    alert_codes: List<Int32>,
    deferred_ids: Option<List<Int32>>
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
        true => { points }
        false => { 0 }
    }
}

fun ready_from_fields: (
    blocked: Boolean,
    estimate: Int32,
    max_estimate: Int32
) -> Boolean = {
    blocked match {
        true => { false }
        false => { estimate <= max_estimate }
    }
}

fun score_from_fields: (
    priority: Int32,
    estimate: Int32,
    blocked: Boolean
) -> Int32 = {
    val blocked_penalty = (blocked, 40) points_when;

    priority * 10 - estimate - blocked_penalty
}

fun route_for: (ready: Boolean, task_id: Int32) -> Result<Int32, Int32> = {
    ready then {
        Ok(task_id)
    } else {
        Err(task_id + 900)
    }
}

fun task_ready: (task: Task, max_estimate: Int32) -> Boolean = {
    val Task {
        id,
        priority,
        estimate,
        blocked,
        owner
    } = task;

    (blocked, estimate, max_estimate) ready_from_fields
}

fun task_card: (
    task: Task,
    max_estimate: Int32,
    fallback_owner: Int32
) -> TaskCard = {
    val Task {
        id,
        priority,
        estimate,
        blocked,
        owner
    } = task;
    val ready = (blocked, estimate, max_estimate) ready_from_fields;
    val score = (priority, estimate, blocked) score_from_fields;
    val owner_id = (owner, fallback_owner) choose_value;
    val route = (ready, id) route_for;

    TaskCard {
        id: id,
        owner: owner_id,
        score: score,
        ready: ready,
        route: route
    }
}

fun first_missing_owner: (
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

fun initial_state: () -> QueueState = {
    QueueState {
        total_score: 0,
        ready_count: 0,
        blocked_count: 0,
        first_unowned: None,
        alert_codes: []
    }
}

fun add_task: (
    state: QueueState,
    task: Task,
    max_estimate: Int32,
    alert_threshold: Int32
) -> QueueState = {
    val QueueState {
        total_score,
        ready_count,
        blocked_count,
        first_unowned,
        alert_codes
    } = state;
    val Task {
        id,
        priority,
        estimate,
        blocked,
        owner
    } = task;
    val score = (priority, estimate, blocked) score_from_fields;
    val ready = (blocked, estimate, max_estimate) ready_from_fields;
    val over_threshold = score >= alert_threshold;
    val next_alerts = over_threshold then {
        (alert_codes, id + 700) list_append
    } else {
        alert_codes
    };

    QueueState {
        total_score: total_score + score,
        ready_count: ready_count + (ready, 1) points_when,
        blocked_count: blocked_count + (blocked, 1) points_when,
        first_unowned: (first_unowned, owner, id) first_missing_owner,
        alert_codes: next_alerts
    }
}

fun empty_plan: () -> QueuePlan = {
    QueuePlan {
        total_score: 0,
        ready_count: 0,
        blocked_count: 0,
        first_unowned: None,
        cards: [],
        alert_codes: [],
        deferred_ids: Some([])
    }
}

fun plan_queue: (
    audit_tasks: List<Task>,
    card_tasks: List<Task>,
    policy: QueuePolicy
) -> QueuePlan = {
    val QueuePolicy {
        max_estimate,
        alert_threshold,
        fallback_owner
    } = policy;
    val ready_tasks = (card_tasks, |task| (task, max_estimate) task_ready) filter;
    val cards = (ready_tasks, |task| (task, max_estimate, fallback_owner) task_card) map;
    val initial = () initial_state;
    val state = (
        audit_tasks,
        initial,
        |current, task| (current, task, max_estimate, alert_threshold) add_task
    ) fold;
    val QueueState {
        total_score,
        ready_count,
        blocked_count,
        first_unowned,
        alert_codes
    } = state;

    QueuePlan {
        total_score: total_score,
        ready_count: ready_count,
        blocked_count: blocked_count,
        first_unowned: first_unowned,
        cards: cards,
        alert_codes: alert_codes,
        deferred_ids: None
    }
}

fun main: () -> QueuePlan = {
    val audit_tasks: List<Task> = [
        Task {
            id: 101,
            priority: 9,
            estimate: 3,
            blocked: false,
            owner: Some(42)
        },
        Task {
            id: 102,
            priority: 5,
            estimate: 13,
            blocked: true,
            owner: None
        }
    ];
    val card_tasks: List<Task> = [
        Task {
            id: 101,
            priority: 9,
            estimate: 3,
            blocked: false,
            owner: Some(42)
        },
        Task {
            id: 103,
            priority: 6,
            estimate: 5,
            blocked: false,
            owner: None
        }
    ];
    val policy = QueuePolicy {
        max_estimate: 8,
        alert_threshold: 70,
        fallback_owner: 7
    };

    (audit_tasks, card_tasks, policy) plan_queue
}
