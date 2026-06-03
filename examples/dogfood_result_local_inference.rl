// Dogfood local Result constructor inference.
// Models a release route snapshot where Ok/Err locals stay unannotated until
// later OSV calls, branches, and record fields force the concrete Result type.

record RouteSnapshot {
    primary: Result<Int32, Int32>,
    fallback: Result<Int32, Int32>,
    selected: Result<Int32, Int32>
}

fun route_score: (route: Result<Int32, Int32>) -> Int32 = {
    route match {
        Ok(owner) => {
            owner
        }
        Err(shortfall) => {
            0 - shortfall
        }
    }
}

fun add_score: (total: Int32, score: Int32) -> Int32 = {
    total + score
}

fun main: () -> Int32 = {
    val release_ready: Boolean = true;
    val primary = Ok(42);
    val fallback = Err(8);
    val primary_copy = primary;
    val selected = release_ready then {
        primary
    } else {
        fallback
    };
    val snapshot = RouteSnapshot {
        primary: primary_copy,
        fallback: fallback,
        selected: selected
    };
    val RouteSnapshot {
        primary: stored_primary,
        fallback: stored_fallback,
        selected: stored_selected
    } = snapshot;
    val scores = [
        stored_primary |> route_score,
        stored_fallback |> route_score,
        stored_selected |> route_score
    ];

    (scores, 0, add_score) fold
}
