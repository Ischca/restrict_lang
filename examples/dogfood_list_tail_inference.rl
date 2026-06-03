// Dogfood Int64 list-tail inference.
// Exercises List<Int64> cons destructuring, record fields with Option/List/Result,
// and folding over the inferred Int64 tail.

record TailSnapshot {
    head: Option<Int64>,
    tail: List<Int64>,
    route: Result<Int64, Int64>
}

fun add_int64: (total: Int64, value: Int64) -> Int64 = {
    total + value
}

fun route_score: (route: Result<Int64, Int64>) -> Int64 = {
    route match {
        Ok(value) => {
            value
        }
        Err(shortfall) => {
            val zero: Int64 = 0;
            zero - shortfall
        }
    }
}

fun snapshot_score: (snapshot: TailSnapshot) -> Int64 = {
    val TailSnapshot {
        head,
        tail,
        route
    } = snapshot;
    val head_score = head match {
        Some(value) => {
            value
        }
        None => {
            val zero: Int64 = 0;
            zero
        }
    };
    val initial: Int64 = 0;
    val tail_score = (tail, initial, add_int64) fold;
    val routed = route |> route_score;

    head_score + tail_score + routed
}

export fun dogfood_list_tail_inference_score: () -> Int64 = {
    val readings: List<Int64> = [
        5_000_000_000,
        6_000_000_000,
        7_000_000_000
    ];
    val snapshot = readings match {
        [] => {
            TailSnapshot {
                head: None,
                tail: [],
                route: Err(3_000_000_000)
            }
        }
        [head | tail] => {
            TailSnapshot {
                head: Some(head),
                tail: tail,
                route: Ok(head)
            }
        }
    };

    snapshot |> snapshot_score
}
