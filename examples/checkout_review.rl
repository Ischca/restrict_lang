// Checkout review example.
// Exercises record destructuring, nested generic expected types, () returns,
// and lambda inference through an expected function parameter.

record Checkout {
    subtotal: Int32,
    expedited: Boolean,
    discount: Option<Int32>,
    risk_flags: Option<List<Int32>>
}

fun expedited_fee: (expedited: Boolean) -> Int32 = {
    expedited match {
        true => { 15 }
        false => { 5 }
    }
}

fun discount_amount: (discount: Option<Int32>) -> Int32 = {
    discount match {
        Some(amount) => { amount }
        None => { 0 }
    }
}

fun risk_surcharge: (flags: Option<List<Int32>>) -> Int32 = {
    flags match {
        Some(items) => { 7 }
        None => { 0 }
    }
}

fun apply_adjustment: (adjust: Int32 -> Int32, amount: Int32) -> Int32 = {
    amount |> adjust
}

fun audit_score: (score: Int32) -> () = {
    ()
}

fun review_checkout: (checkout: Checkout) -> Int32 = {
    val Checkout { subtotal, expedited, discount, risk_flags } = checkout;
    val shipping = expedited |> expedited_fee;
    val discount_value = discount |> discount_amount;
    val risk = risk_flags |> risk_surcharge;
    val after_discount = subtotal - discount_value;

    (|amount| amount + shipping + risk, after_discount) apply_adjustment
}

fun main: () -> Int32 = {
    val sample: Checkout = Checkout {
        subtotal: 120,
        expedited: true,
        discount: None,
        risk_flags: Some([])
    };

    sample |> review_checkout
}
