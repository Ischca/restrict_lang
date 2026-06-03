// Dogfood nested Result/List generic record inference.
// Models a billing import where unannotated meter deltas flow through
// Result<List<T>, String> and are later consumed as List<Int64>.

record UsageImport<T> {
    account_id: Int32,
    meter_deltas: Result<List<T>, String>
}

fun add_int64: (total: Int64, value: Int64) -> Int64 = {
    total + value
}

fun billable_delta_score: (deltas: List<Int64>) -> Int64 = {
    val zero: Int64 = 0;
    (deltas, zero, add_int64) fold
}

export fun dogfood_nested_result_list_score: () -> Int64 = {
    val batch = UsageImport {
        account_id: 42,
        meter_deltas: Ok([
            5_000_000_000,
            6_000_000_000,
            7_000_000_000
        ])
    };

    batch.meter_deltas match {
        Ok(deltas) => {
            deltas |> billable_delta_score
        }
        Err(message) => {
            val zero: Int64 = 0;
            zero
        }
    }
}
