// Dogfoods v0.0.1 Array and Range inference in a release-style workflow.
// The exported surface stays primitive while the source model uses composite values.

record ReleaseWindow {
    span: Range<Int32>,
    checkpoints: Array<Int32, 3>,
    fallback: Array<Option<Int32>, 2>
}

fun make_window: (base: Int32) -> ReleaseWindow = {
    ReleaseWindow {
        span: [base..base + 4],
        checkpoints: [base, base + 2, base + 4],
        fallback: [None, Some(base)]
    }
}

export fun score_release_window: (risk: Int32) -> Int32 = {
    with Arena {
        val plan = risk |> make_window;
        val ReleaseWindow { checkpoints, fallback, ..._ } = plan;
        mut val working = checkpoints;
        (working, 1, risk + 7) array_set;
        val middle = (working, 1) array_get;
        val reviewer = (fallback, 1) array_get;

        reviewer match {
            Some(value) => { middle + value }
            None => { middle }
        }
    }
}
