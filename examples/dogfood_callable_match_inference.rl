// Dogfoods callable match-arm inference.
// Models a support-priority workflow where an optional mapper chooses either
// an existing function value or an immediate lambda for final score adjustment.

fun incident_boost: (score: Int32) -> Int32 = {
    score + 40
}

fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun choose_mapper: (override_mapper: Option<Int32 -> Int32>) -> Int32 -> Int32 = {
    override_mapper match {
        Some(mapper) => {
            mapper
        }
        None => {
            |score| score + 5
        }
    }
}

fun prioritize_ticket: (
    base_score: Int32,
    workflow_mapper: Option<Int32 -> Int32>,
    audit_codes: List<Int32>
) -> Int32 = {
    val mapper = workflow_mapper |> choose_mapper;
    val adjusted_score = base_score |> mapper;
    val audit_score = (audit_codes, 0, add_int) fold;

    adjusted_score + audit_score
}

fun main: () -> Int32 = {
    val urgent_mapper: Option<Int32 -> Int32> = Some(incident_boost);
    val normal_mapper: Option<Int32 -> Int32> = None;
    val urgent_score = (10, urgent_mapper, [1, 2]) prioritize_ticket;
    val normal_score = (20, normal_mapper, [3]) prioritize_ticket;

    urgent_score + normal_score
}
