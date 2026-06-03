// Status routing example.
// Dogfoods Char/String/Float64 literal patterns, records, and Boolean routing logic.

record QueueItem {
    priority: Char,
    status: String,
    load_factor: Float64,
    open: Boolean,
    age_hours: Int32
}

fun priority_score: (priority: Char) -> Int32 = {
    priority match {
        'H' => {
            3
        }
        'M' => {
            2
        }
        _ => {
            1
        }
    }
}

fun status_score: (status: String) -> Int32 = {
    status match {
        "outage" => {
            4
        }
        "degraded" => {
            2
        }
        _ => {
            0
        }
    }
}

fun load_score: (load_factor: Float64) -> Int32 = {
    load_factor match {
        0.0 => {
            0
        }
        1.0 => {
            2
        }
        _ => {
            1
        }
    }
}

fun should_page: (item: QueueItem) -> Boolean = {
    val QueueItem { priority, status, ...routing } = item;
    val priority_points = priority |> priority_score;
    val status_points = status |> status_score;
    val load_points = routing.load_factor |> load_score;
    val high_priority = priority_points >= 3;
    val hard_down = status_points >= 4;
    val saturated = load_points >= 2;
    val stale = routing.age_hours > 24;
    val closed = !routing.open;
    val active = !closed;
    active && (high_priority || hard_down || saturated || stale)
}

fun main: () -> Int32 = {
    val item = QueueItem {
        priority: 'H',
        status: "outage",
        load_factor: 1.0,
        open: true,
        age_hours: 4
    };

    val page = item |> should_page;
    page then {
        1
    } else {
        0
    }
}
