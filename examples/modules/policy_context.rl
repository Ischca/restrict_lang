// Policy context module.
// Keeps limits and impl methods module-local while exposing a scalar decision API.

export record ReviewSignal {
    failures: Int32,
    stale_days: Int32
}

export record RolloutSignal {
    exposure_percent: Int32,
    canary_failures: Int32
}

record PolicyLimits {
    minimum_score: Int32,
    failure_penalty: Int32,
    stale_penalty: Int32,
    canary_penalty: Int32,
    exposure_penalty: Int32
}

context ReviewPolicy {
    limits: PolicyLimits
}

impl ReviewSignal {
    fun policy_penalty: (
        self: ReviewSignal,
        major_penalty: Int32,
        minor_penalty: Int32
    ) -> Int32 = {
        (self.failures * major_penalty) + (self.stale_days * minor_penalty)
    }
}

impl RolloutSignal {
    fun policy_penalty: (
        self: RolloutSignal,
        major_penalty: Int32,
        minor_penalty: Int32
    ) -> Int32 = {
        (self.canary_failures * major_penalty) + (self.exposure_percent * minor_penalty)
    }
}

export fun decide_review: (
    review: ReviewSignal,
    rollout: RolloutSignal,
    base_score: Int32
) -> Int32 = {
    with ReviewPolicy {
        limits: PolicyLimits {
            minimum_score: 50,
            failure_penalty: 12,
            stale_penalty: 2,
            canary_penalty: 20,
            exposure_penalty: 1
        }
    } {
        val PolicyLimits {
            minimum_score,
            failure_penalty,
            stale_penalty,
            canary_penalty,
            exposure_penalty
        } = limits;
        val review_penalty = (review, failure_penalty, stale_penalty) policy_penalty;
        val rollout_penalty = (rollout, canary_penalty, exposure_penalty) policy_penalty;
        val raw_score = base_score - review_penalty - rollout_penalty;

        raw_score >= minimum_score then {
            raw_score
        } else {
            minimum_score
        }
    }
}
