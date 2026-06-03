// Dogfoods impl dispatch through expected lambdas and a generic impl method.
// The exported ABI stays scalar while source code uses records, Option, and List.

record ServiceSignal {
    failures: Int32,
    latency_ms: Int32,
    owner: Option<Int32>
}

record RolloutSignal {
    canary_failures: Int32,
    exposure_percent: Int32,
    owner: Option<Int32>
}

record Keeper {
    seed: Int32
}

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun owner_or: (owner: Option<Int32>, fallback: Int32) -> Int32 = {
    owner match {
        Some(person) => { person }
        None => { fallback }
    }
}

impl ServiceSignal {
    fun risk: (self: ServiceSignal, fallback_owner: Int32) = {
        val ServiceSignal { failures, latency_ms, owner } = self
        val owner_score = (owner, fallback_owner) owner_or

        failures * 20 + latency_ms + owner_score
    }
}

impl RolloutSignal {
    fun risk: (self: RolloutSignal, fallback_owner: Int32) = {
        val RolloutSignal { canary_failures, exposure_percent, owner } = self
        val owner_score = (owner, fallback_owner) owner_or

        canary_failures * 30 + exposure_percent + owner_score
    }
}

impl Keeper {
    fun keep: <T>(self: Keeper, value: T) -> T = {
        value
    }
}

export fun impl_dispatch_inference_score: () -> Int32 = {
    val service_scores = ([
        ServiceSignal { failures: 2, latency_ms: 15, owner: Some(4) }
    ], |signal| (signal, 3) risk) map
    val rollout_scores = ([
        RolloutSignal { canary_failures: 1, exposure_percent: 20, owner: None }
    ], |signal| (signal, 5) risk) map

    val service_total = (service_scores, 0, add_int) fold
    val rollout_total = (rollout_scores, 0, add_int) fold
    val keeper = Keeper { seed: 1 }
    val kept = (keeper, Some(service_total)) keep
    val kept_score = kept match {
        Some(score) => { score }
        None => { 0 }
    }

    service_total + rollout_total + kept_score
}
