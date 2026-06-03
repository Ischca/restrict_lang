// Dogfoods deferred branch callable inference with replay-safe prefix bindings.
// Models a release scoring pass where branch and match blocks produce mappers
// whose final lambda type is learned later from List.map.

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun score_batch: (
    emergency: Boolean,
    manual_bonus: Option<Int32>,
    scores: List<Int32>
) -> Int32 = {
    val adjust = emergency then {
        val boost = 2;
        |score| score + boost
    } else {
        val factor = 2;
        |score| score * factor
    };
    val normalize = manual_bonus match {
        Some(bonus) => {
            val doubled = bonus * 2;
            |score| score + doubled
        }
        None => {
            val doubled = 0;
            |score| score + doubled
        }
    };
    val adjusted = (scores, adjust) map;
    val normalized = (adjusted, normalize) map;

    (normalized, 0, add_int) fold
}

fun main: () -> Int32 = {
    val scores = [10, 20];

    (true, Some(3), scores) score_batch
}
