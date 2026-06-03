// Release review digest example.
// Dogfoods nested record destructuring with Option and exact List patterns.

record OwnerSignal {
    confidence: Option<Int32>,
    queue: List<Int32>
}

record ReleaseSlice {
    owner: OwnerSignal,
    blocker_codes: List<Int32>,
    fallback: Int32
}

record Digest {
    priority: Int32,
    confidence: Int32,
    first_blocker: Int32,
    second_blocker: Int32
}

fun summarize_slice: (slice: ReleaseSlice) -> Digest = {
    val ReleaseSlice {
        owner: OwnerSignal {
            confidence: Some(confidence),
            queue: [first_queue, second_queue]
        },
        blocker_codes: [first_blocker, second_blocker],
        fallback
    } = slice;

    Digest {
        priority: first_queue + second_queue + fallback,
        confidence: confidence,
        first_blocker: first_blocker,
        second_blocker: second_blocker
    }
}

fun main: () -> Digest = {
    val slice = ReleaseSlice {
        owner: OwnerSignal {
            confidence: Some(8),
            queue: [3, 5]
        },
        blocker_codes: [101, 202],
        fallback: 2
    };

    slice |> summarize_slice
}
