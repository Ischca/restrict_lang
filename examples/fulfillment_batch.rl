// Fulfillment batch example.
// Dogfoods a practical shipment planning workflow with records, Option flow,
// map/filter/fold, empty list inference, None inference, and expected-type
// lambda inference. The two shipment lists are intentionally separate because
// List values are affine and each pipeline consumes its input.

record Shipment {
    id: Int32,
    zone: Int32,
    value: Int32,
    fragile: Boolean,
    driver: Option<Int32>
}

record FulfillmentState {
    priority_total: Int32,
    fragile_count: Int32,
    first_unassigned: Option<Int32>,
    manual_count: Int32
}

record FulfillmentPlan {
    priority_total: Int32,
    fragile_count: Int32,
    first_unassigned: Option<Int32>,
    manual_count: Int32,
    manual_scores: List<Int32>,
    audit_codes: List<Int32>
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun driver_missing: (driver: Option<Int32>) -> Boolean = {
    driver match {
        Some(person) => { false }
        None => { true }
    }
}

fun shipment_priority: (shipment: Shipment) -> Int32 = {
    val Shipment { id, zone, value, fragile, driver } = shipment;
    val fragile_points = (fragile, 12) points_when;
    val missing_driver = driver |> driver_missing;
    val assignment_points = (missing_driver, 20) points_when;

    value + zone * 3 + fragile_points + assignment_points
}

fun needs_manual_review: (shipment: Shipment) -> Boolean = {
    val Shipment { id, zone, value, fragile, driver } = shipment;
    val missing_driver = driver |> driver_missing;
    val high_value = value > 70;

    high_value match {
        true => { true }
        false => { missing_driver }
    }
}

fun manual_scores_for: (shipments: List<Shipment>) -> List<Int32> = {
    val manual_shipments = (shipments, |shipment| shipment |> needs_manual_review) filter;
    (manual_shipments, |shipment| shipment |> shipment_priority) map
}

fun first_unassigned_driver: (
    current: Option<Int32>,
    driver: Option<Int32>,
    shipment_id: Int32
) -> Option<Int32> = {
    current match {
        Some(existing) => { Some(existing) }
        None => {
            driver match {
                Some(person) => { None }
                None => { Some(shipment_id) }
            }
        }
    }
}

fun add_shipment: (state: FulfillmentState, shipment: Shipment) -> FulfillmentState = {
    val FulfillmentState {
        priority_total,
        fragile_count,
        first_unassigned,
        manual_count
    } = state;
    val Shipment { id, zone, value, fragile, driver } = shipment;
    val missing_driver = driver |> driver_missing;
    val manual_needed = value > 70 then {
        true
    } else {
        missing_driver
    };

    FulfillmentState {
        priority_total: priority_total + value + zone * 3 + (fragile, 12) points_when,
        fragile_count: fragile_count + (fragile, 1) points_when,
        first_unassigned: (first_unassigned, driver, id) first_unassigned_driver,
        manual_count: manual_count + (manual_needed, 1) points_when
    }
}

fun build_plan: (
    aggregate_shipments: List<Shipment>,
    score_shipments: List<Shipment>,
    fallback_codes: List<Int32>
) -> FulfillmentPlan = {
    val initial = FulfillmentState {
        priority_total: 0,
        fragile_count: 0,
        first_unassigned: None,
        manual_count: 0
    };
    val final_state = (aggregate_shipments, initial, |state, shipment| (state, shipment) add_shipment) fold;
    val FulfillmentState {
        priority_total,
        fragile_count,
        first_unassigned,
        manual_count
    } = final_state;
    val manual_scores = score_shipments |> manual_scores_for;
    val audit_codes: List<Int32> = [] |> (|codes| (codes, fallback_codes) choose_first);
    val missing_driver: Option<Int32> = None |> (|empty| (empty, first_unassigned) choose_first);

    FulfillmentPlan {
        priority_total: priority_total,
        fragile_count: fragile_count,
        first_unassigned: missing_driver,
        manual_count: manual_count,
        manual_scores: manual_scores,
        audit_codes: audit_codes
    }
}

fun main: () -> FulfillmentPlan = {
    val aggregate_shipments: List<Shipment> = [
        Shipment {
            id: 301,
            zone: 2,
            value: 65,
            fragile: true,
            driver: Some(4)
        },
        Shipment {
            id: 302,
            zone: 5,
            value: 81,
            fragile: false,
            driver: None
        }
    ];
    val score_shipments: List<Shipment> = [
        Shipment {
            id: 401,
            zone: 1,
            value: 92,
            fragile: true,
            driver: Some(8)
        },
        Shipment {
            id: 402,
            zone: 3,
            value: 44,
            fragile: false,
            driver: None
        }
    ];
    val fallback_codes = [710, 711];

    (aggregate_shipments, score_shipments, fallback_codes) build_plan
}
