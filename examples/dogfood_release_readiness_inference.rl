// Dogfood release readiness inference example.
// Exercises non-TAT local expected type inference with an unannotated mapper,
// collection inference through map/filter/fold, Option matching, Result
// matching, and OSV calls.

fun choose_or: <T>(maybe: Option<T>, fallback: T) -> T = {
    maybe match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun classify_release: (score: Int32) -> Result<Int32, Int32> = {
    score >= 250 then {
        Ok(score)
    } else {
        Err(250 - score)
    }
}

fun main: () -> Int32 = {
    val check_scores = [91, 76, 88, 65];
    val apply_buffer = |score| score + 5;
    val buffered_scores = (check_scores, apply_buffer) map;
    val passing_scores = (buffered_scores, |score| score >= 80) filter;
    val total_score = (passing_scores, 0, |total, score| total + score) fold;
    val manual_override: Option<Int32> = None;
    val final_score = (manual_override, total_score) choose_or;
    val release = final_score |> classify_release;

    release match {
        Ok(score) => {
            score
        }
        Err(shortfall) => {
            0 - shortfall
        }
    }
}
