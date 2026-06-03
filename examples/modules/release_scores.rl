// Internal release scoring helpers used by release_policy.

export fun score_signal: (signal: Int32) -> Int32 = {
    signal * 2
}

export fun add_signal_score: (total: Int32, score: Int32) -> Int32 = {
    total + score
}
