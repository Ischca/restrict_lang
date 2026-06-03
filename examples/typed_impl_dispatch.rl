// Typed impl dispatch example.
// Dogfoods same-name impl methods as OSV functions selected by receiver type,
// without introducing object.method syntax.

record HealthSignal {
    errors: Int32,
    latency: Float64
}

record RolloutSignal {
    canary_pass_rate: Float64,
    exposure: Float64
}

record DispatchDecision {
    health_risk: Float64,
    rollout_risk: Float64,
    approved: Boolean
}

impl HealthSignal {
    fun risk_score: (self: HealthSignal, bias: Float64) -> Float64 = {
        val error_risk = self.errors > 0 then {
            40.0
        } else {
            0.0
        };

        error_risk + self.latency + bias
    }
}

impl RolloutSignal {
    fun risk_score: (self: RolloutSignal, bias: Float64) -> Float64 = {
        val canary_risk = self.canary_pass_rate < 95.0 then {
            25.0
        } else {
            0.0
        };

        canary_risk + (self.exposure * 10.0) + bias
    }
}

fun decide_dispatch: (
    health: HealthSignal,
    rollout: RolloutSignal,
    risk_bias: Float64
) -> DispatchDecision = {
    val health_risk = (health, risk_bias) risk_score;
    val rollout_risk = (rollout, risk_bias) risk_score;
    val total_risk = health_risk + rollout_risk;

    DispatchDecision {
        health_risk: health_risk,
        rollout_risk: rollout_risk,
        approved: total_risk < 90.0
    }
}

fun main: () -> DispatchDecision = {
    val health = HealthSignal {
        errors: 1,
        latency: 18.5
    };
    val rollout = RolloutSignal {
        canary_pass_rate: 97.0,
        exposure: 0.5
    };

    (health, rollout, 3.0) decide_dispatch
}
