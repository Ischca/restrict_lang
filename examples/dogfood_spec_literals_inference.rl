// Small non-TAT dogfood for spec literal forms and inference.
// Exercises pub exports, hex/underscored/exponent/escaped literals,
// colon-only record fields, Option/Result/List inference, and OSV calls.

pub val score_bias: Int32 = 0x10

record LiteralProfile {
    name: String,
    separator: Char,
    scale: Float64,
    base: Int32,
    samples: List<Int32>,
    owner: Option<Int32>
}

record LiteralPlan {
    score: Int32,
    route: Result<Int32, Int32>,
    audit_ids: List<Int32>,
    owner_seen: Option<Int32>
}

pub fun exported_bias: () -> Int32 = {
    score_bias
}

fun choose_value: <T>(preferred: Option<T>, fallback: T) -> T = {
    preferred match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun escape_bonus: (separator: Char) -> Int32 = {
    separator match {
        '\n' => {
            10
        }
        '\t' => {
            9
        }
        '\\' => {
            92
        }
        '\'' => {
            39
        }
        _ => {
            0
        }
    }
}

fun scale_bonus: (scale: Float64) -> Int32 = {
    scale > 3.14E-2 then {
        5
    } else {
        0
    }
}

fun sum_samples: (samples: List<Int32>) -> Int32 = {
    (samples, 0, add_int) fold
}

fun route_score: (score: Int32) -> Result<Int32, Int32> = {
    score < 1_000_000 then {
        Ok(score)
    } else {
        Err(score)
    }
}

fun plan_profile: (profile: LiteralProfile) -> LiteralPlan = {
    val LiteralProfile {
        name,
        separator,
        scale,
        base,
        samples,
        owner
    } = profile;
    val sample_score = samples |> sum_samples;
    val owner_id = (owner, 0x2A) choose_value;
    val score = base + score_bias + sample_score + (separator |> escape_bonus) + (scale |> scale_bonus) + owner_id;
    val route = score |> route_score;

    LiteralPlan {
        score: score,
        route: route,
        audit_ids: [],
        owner_seen: Some(owner_id)
    }
}

fun main: () -> LiteralPlan = {
    val profile = LiteralProfile {
        name: "alpha\nbeta\t\\\"\'",
        separator: '\n',
        scale: 1.5e10,
        base: 0xFF,
        samples: [1_000, 2, 3],
        owner: None
    };

    profile |> plan_profile
}
