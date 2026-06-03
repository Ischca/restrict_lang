// Release queue snapshot example.
// Dogfoods std list accessors as OSV calls: empty check, head/tail as Option,
// reverse, and record payloads flowing through match.

record Candidate {
    priority: Int32,
    risk: Int32,
    effort: Int32
}

fun candidate_score: (candidate: Candidate) -> Int32 = {
    val Candidate {
        priority,
        risk,
        effort
    } = candidate;

    (priority * 3) + risk - effort
}

fun score_candidate_option: (maybe_candidate: Option<Candidate>) -> Int32 = {
    maybe_candidate match {
        Some(candidate) => {
            candidate |> candidate_score
        }
        None => {
            0
        }
    }
}

fun count_tail_option: (maybe_tail: Option<List<Candidate>>) -> Int32 = {
    maybe_tail match {
        Some(tail) => {
            tail |> list_count
        }
        None => {
            0
        }
    }
}

fun main: () -> Int32 = {
    mut val candidates: List<Candidate> = [
        Candidate {
            priority: 5,
            risk: 2,
            effort: 3
        },
        Candidate {
            priority: 3,
            risk: 6,
            effort: 5
        },
        Candidate {
            priority: 4,
            risk: 1,
            effort: 2
        }
    ];

    val empty = candidates |> list_is_empty;
    mut val lead = candidates |> list_head;
    val remaining = candidates |> list_tail;
    val review_order = candidates |> list_reverse;

    val lead_exists = lead |> option_is_some;
    val lead_score = lead |> score_candidate_option;
    val remaining_count = remaining |> count_tail_option;
    val last_reviewed = review_order |> list_head;
    val last_score = last_reviewed |> score_candidate_option;

    empty then {
        0
    } else {
        lead_exists then {
            lead_score + remaining_count + last_score
        } else {
            0
        }
    }
}
