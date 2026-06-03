// Calibration pipeline example.
// Builds a small sensor-quality score and dogfoods Float64 map/filter/fold,
// Option map/filter, and captured Float64 values inside closures.

fun normalize_sample: (raw: Int32, baseline: Float64) -> Float64 = {
    raw > 800 then {
        baseline + 2.5
    } else {
        raw < 100 then {
            baseline - 1.5
        } else {
            baseline + 0.25
        }
    }
}

fun sum_drift: (total: Float64, sample: Float64) -> Float64 = {
    total + sample
}

fun quality_score: (
    raw_samples: List<Int32>,
    manual_offset: Option<Float64>,
    baseline: Float64,
    threshold: Float64
) -> Float64 = {
    val normalized = (raw_samples, |raw| (raw, baseline) normalize_sample) map;
    val outliers = (normalized, |sample| sample > threshold) filter;
    val drift_total = (outliers, 0.0, sum_drift) fold;
    val safe_offset = (manual_offset, |offset| offset + baseline) map;
    val accepted_offset = (safe_offset, |offset| offset < threshold) filter;

    accepted_offset match {
        Some(offset) => {
            drift_total + offset
        }
        None => {
            drift_total
        }
    }
}

fun main: () -> Float64 = {
    val raw_samples = [42, 512, 901];
    val manual_offset: Option<Float64> = Some(0.75);
    (raw_samples, manual_offset, 10.0, 11.0) quality_score
}
