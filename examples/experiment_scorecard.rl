// Experiment scorecard example.
// Dogfoods a small analytics decision engine with records, Float64 pipelines,
// first-class functions returned from factories, list map/filter/fold, Option
// map/filter, and direct calls through typed function values.

record Metric {
    exposure: Int32,
    conversions: Int32,
    latency_ms: Float64,
    revenue: Float64
}

record ExperimentDecision {
    preview_score: Float64,
    passing_total: Float64,
    accepted: Boolean,
    fallback_bonus: Option<Float64>,
    normalized_scores: List<Float64>
}

record ScoreRulebook {
    score_metric: Metric -> Float64,
    score_passes: Float64 -> Boolean
}

fun add_score: (total: Float64, score: Float64) -> Float64 = {
    total + score
}

fun normalize_score: (score: Float64, divisor: Float64) -> Float64 = {
    score / divisor
}

fun metric_score_with: (metric: Metric, baseline: Float64) -> Float64 = {
    val Metric {
        exposure,
        conversions,
        latency_ms,
        revenue
    } = metric;
    val conversion_score = conversions > 50 then {
        revenue + baseline
    } else {
        baseline - 1.0
    };
    val exposure_penalty = exposure < 1000 then {
        2.0
    } else {
        0.0
    };

    conversion_score - exposure_penalty - (latency_ms / 1000.0)
}

fun make_metric_scorer: (baseline: Float64) -> Metric -> Float64 = {
    val scorer: Metric -> Float64 = |metric| {
        (metric, baseline) metric_score_with
    };

    scorer
}

fun make_passing_rule: (minimum: Float64) -> Float64 -> Boolean = {
    val passing: Float64 -> Boolean = |score| score >= minimum;
    passing
}

fun build_rulebook: (baseline: Float64, minimum: Float64) -> ScoreRulebook = {
    ScoreRulebook {
        score_metric: |metric| (metric, baseline) metric_score_with,
        score_passes: |score| score >= minimum
    }
}

fun evaluate_experiment: (
    scoring_metrics: List<Metric>,
    report_metrics: List<Metric>,
    preview_metric: Metric,
    fallback_input: Option<Float64>,
    baseline: Float64,
    minimum: Float64
) -> ExperimentDecision = {
    val preview_rulebook = (baseline, minimum) build_rulebook;
    val preview_scorer = preview_rulebook.score_metric;
    val preview_score = preview_metric |> preview_scorer;

    val ScoreRulebook {
        score_metric,
        score_passes
    } = (baseline, minimum) build_rulebook;
    val raw_scores = (scoring_metrics, score_metric) map;
    val passing_scores = (raw_scores, score_passes) filter;
    val passing_total = (passing_scores, 0.0, add_score) fold;

    val report_rulebook = (baseline, minimum) build_rulebook;
    val report_scores = (report_metrics, report_rulebook.score_metric) map;
    val normalized_scores = (report_scores, |score| (score, minimum) normalize_score) map;
    val fallback_bonus = (fallback_input, |value| value + baseline) map;
    val accepted = passing_total > minimum;

    ExperimentDecision {
        preview_score: preview_score,
        passing_total: passing_total,
        accepted: accepted,
        fallback_bonus: (fallback_bonus, |bonus| bonus > 0.0) filter,
        normalized_scores: normalized_scores
    }
}

fun main: () -> ExperimentDecision = {
    val preview_metric = Metric {
        exposure: 800,
        conversions: 64,
        latency_ms: 120.0,
        revenue: 8.5
    };
    val scoring_metrics: List<Metric> = [
        Metric {
            exposure: 1200,
            conversions: 90,
            latency_ms: 80.0,
            revenue: 12.5
        },
        Metric {
            exposure: 500,
            conversions: 20,
            latency_ms: 220.0,
            revenue: 4.0
        }
    ];
    val report_metrics: List<Metric> = [
        Metric {
            exposure: 1300,
            conversions: 75,
            latency_ms: 70.0,
            revenue: 10.0
        },
        Metric {
            exposure: 900,
            conversions: 55,
            latency_ms: 110.0,
            revenue: 7.5
        }
    ];
    val Some(seed_bonus) = Some(1.5);
    val fallback_input: Option<Float64> = Some(seed_bonus);

    (scoring_metrics, report_metrics, preview_metric, fallback_input, 3.0, 5.0) evaluate_experiment
}
