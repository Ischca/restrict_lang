// Dogfoods built-in Option Container inference for a review policy workflow.
// The unannotated lambdas rely on Option map/filter expected-type propagation.

record ReviewPolicyInput {
    base_risk: Int32,
    reviewer_load: Option<Int32>,
    customer_data: Boolean
}

fun points_when: (flag: Boolean, points: Int32) -> Int32 = {
    flag match {
        true => { points }
        false => { 0 }
    }
}

fun review_policy_score: (input: ReviewPolicyInput) -> Int32 = {
    val ReviewPolicyInput {
        base_risk,
        reviewer_load,
        customer_data
    } = input;
    val privacy_points = (customer_data, 12) points_when;
    val adjusted = (reviewer_load, |load| load + base_risk) map;
    val actionable = (adjusted, |score| score >= 70) filter;

    actionable match {
        Some(score) => {
            score + privacy_points
        }
        None => {
            base_risk + privacy_points
        }
    }
}

export fun dogfood_option_container_lambda_score: (
    base_risk: Int32,
    has_reviewer_load: Boolean,
    customer_data: Boolean
) -> Int32 = {
    val reviewer_load: Option<Int32> = has_reviewer_load then {
        Some(18)
    } else {
        None
    };
    val input = ReviewPolicyInput {
        base_risk: base_risk,
        reviewer_load: reviewer_load,
        customer_data: customer_data
    };

    input |> review_policy_score
}
