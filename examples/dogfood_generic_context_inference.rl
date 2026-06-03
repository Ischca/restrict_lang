// Dogfood scoped generic context inference.
// Models a rollout review where unannotated empty success and optional override
// locals flow through a user generic helper before container operations force
// their concrete List/Option/Result types.

fun choose_primary: <T>(value: T, fallback: T) -> T = {
    value
}

fun add_score: (total: Int32, score: Int32) -> Int32 = {
    total + score
}

fun score_result_batch: (batch: Result<List<Int32>, String>) -> Int32 = {
    batch match {
        Ok(scores) => {
            (scores, 0, add_score) fold
        }
        Err(message) => {
            0
        }
    }
}

fun option_score: (maybe_override: Option<Int32>) -> Int32 = {
    val boosted = (maybe_override, |score| score + 10) map;
    val gated = (boosted, |score| score > 10) filter;

    gated match {
        Some(score) => {
            score
        }
        None => {
            3
        }
    }
}

export fun dogfood_generic_context_score: () -> Int32 = {
    val empty_batch = Ok([]);
    val fallback_batch = Ok([4, 5]);
    val batch_score = ((empty_batch, fallback_batch) choose_primary) |> score_result_batch;

    val missing_override = None;
    val fallback_override = Some(12);
    val override_score = ((missing_override, fallback_override) choose_primary) |> option_score;

    batch_score + override_score
}
