fun latest_checkpoint_id: () -> Int64 = {
    mut val checkpoint_ids = [];
    checkpoint_ids = (checkpoint_ids, 10000000000) list_append;
    checkpoint_ids = (checkpoint_ids, 20000000000) list_append;
    (checkpoint_ids, 1) list_get
}

fun selected_ratio: () -> Float64 = {
    mut val ratio = None;
    ratio = Some(1.5);

    ratio match {
        Some(value) => {
            value + 0.25
        }
        None => {
            0.0
        }
    }
}

export fun checkpoint_id_score: () -> Int64 = {
    () latest_checkpoint_id
}

export fun checkpoint_ratio_score: () -> Float64 = {
    () selected_ratio
}
