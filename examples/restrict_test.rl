// v0.0.1 Restrict smoke example.
// Replaces the removed fluent test DSL with direct release-gate data flow.

record Scenario {
    score: Int32,
    covered: Boolean,
    owner: Option<Int32>
}

record Decision {
    approved: Boolean,
    code: Int32
}

fun owner_penalty: (owner: Option<Int32>) -> Int32 = {
    owner match {
        Some(person) => { 0 }
        None => { 50 }
    }
}

fun decide: (scenario: Scenario) -> Decision = {
    val Scenario { score, covered, owner } = scenario;
    val penalty = owner |> owner_penalty;
    val code = score + penalty;

    Decision {
        approved: covered && code < 80,
        code: code
    }
}

fun main: () -> Int32 = {
    val scenario = Scenario {
        score: 25,
        covered: true,
        owner: Some(7)
    };
    val decision = scenario |> decide;

    decision.code
}
