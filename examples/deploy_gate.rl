// Deploy gate example.
// A small release-readiness policy that exercises records, nested options,
// empty list inference, OSV calls, and generic map lambda inference.

record CheckRun {
    failures: Int32,
    warnings: Int32,
    flaky: Boolean
}

record PullRequest {
    checks: CheckRun,
    review_score: Option<Int32>,
    risk_flags: Option<List<Int32>>,
    touched_modules: List<Int32>
}

record GateReport {
    release_score: Int32,
    blocker_codes: List<Int32>,
    needs_review: Option<Int32>
}

fun bool_penalty: (flag: Boolean, penalty: Int32) -> Int32 = {
    flag match {
        true => { penalty }
        false => { 0 }
    }
}

fun review_points: (score: Option<Int32>) -> Int32 = {
    score match {
        Some(points) => { points }
        None => { 0 }
    }
}

fun risk_penalty: (flags: Option<List<Int32>>) -> Int32 = {
    flags match {
        Some(items) => {
            val total = (items, 0, |acc, flag| acc + flag) fold;
            total + 20
        }
        None => { 0 }
    }
}

fun severity_code: (module: Int32) -> Int32 = {
    module + 100
}

fun maybe_review: (score: Int32) -> Option<Int32> = {
    score < 75 then {
        Some(score)
    } else {
        None
    }
}

fun gate_pull_request: (pr: PullRequest) -> GateReport = {
    val PullRequest { checks, review_score, risk_flags, touched_modules } = pr;
    val CheckRun { failures, warnings, flaky } = checks;
    val failure_penalty = failures * 30;
    val warning_penalty = warnings * 3;
    val flaky_penalty = (flaky, 15) bool_penalty;
    val risk = risk_flags |> risk_penalty;
    val review = review_score |> review_points;
    val score = 100 + review - failure_penalty - warning_penalty - flaky_penalty - risk;
    val touched_core_modules = (touched_modules, |module| module > 1) filter;
    val blockers: List<Int32> = (touched_core_modules, |module| module |> severity_code) map;

    GateReport {
        release_score: score,
        blocker_codes: blockers,
        needs_review: score |> maybe_review
    }
}

fun main: () -> GateReport = {
    val pr: PullRequest = PullRequest {
        checks: CheckRun {
            failures: 0,
            warnings: 2,
            flaky: true
        },
        review_score: Some(10),
        risk_flags: Some([]),
        touched_modules: [1, 2, 3]
    };

    pr |> gate_pull_request
}
