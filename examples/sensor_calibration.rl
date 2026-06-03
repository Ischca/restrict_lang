// Sensor calibration example.
// Dogfoods Float64 lists, record destructuring, local inference, Option matching, and OSV calls.

record SensorReading {
    measured: Float64,
    expected: Float64,
    tolerance: Float64
}

fun drift: (reading: SensorReading) -> Float64 = {
    val SensorReading { measured, expected, tolerance } = reading;
    val delta = measured > expected then {
        measured - expected
    } else {
        expected - measured
    };

    val outside_tolerance = delta > tolerance;
    outside_tolerance match {
        true => {
            delta
        }
        false => {
            0.0
        }
    }
}

fun apply_offset: (value: Float64, offset: Option<Float64>) -> Float64 = {
    offset match {
        Some(amount) => {
            value + amount
        }
        None => {
            value
        }
    }
}

fun calibration_offset: (needs_adjustment: Boolean) -> Option<Float64> = {
    needs_adjustment match {
        true => {
            Some(-0.25)
        }
        false => {
            None
        }
    }
}

fun main: () -> Float64 = {
    mut val samples: List<Float64> = [73.8, 73.5, 72.9];
    val sample_count = samples |> list_length;
    val first_sample = (samples, 0) list_get;

    val reading = SensorReading {
        measured: first_sample,
        expected: 72.0,
        tolerance: 1.0
    };

    val raw = reading |> drift;
    val normalized_raw = raw % 10.0;
    val enough_samples = sample_count > 0;
    val needs_adjustment = (normalized_raw > 0.0) && enough_samples;
    val offset = needs_adjustment |> calibration_offset;
    (normalized_raw, offset) apply_offset
}
