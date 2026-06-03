// Subscription billing example.
// Exercises record destructuring, Option matching, List patterns, zero-argument
// lambdas, and empty-list inference through a function return context.

record Plan {
    base: Int32,
    per_seat: Int32,
    trial: Boolean
}

record Usage {
    seats: Int32,
    incidents: Int32,
    coupon: Option<Int32>,
    credits: List<Int32>
}

record Invoice {
    subtotal: Int32,
    discount: Int32,
    total: Int32,
    review: Boolean
}

fun default_credits: () -> List<Int32> = {
    val credits = [];
    credits
}

fun trial_credit: (trial: Boolean) -> Int32 = {
    trial match {
        true => { 25 }
        false => { 0 }
    }
}

fun coupon_discount: (coupon: Option<Int32>, fallback: () -> Int32) -> Int32 = {
    coupon match {
        Some(amount) => { amount }
        None => { () fallback }
    }
}

fun incident_surcharge: (incidents: Int32) -> Int32 = {
    incidents > 5 then {
        40
    } else {
        0
    }
}

fun first_credit: (credits: List<Int32>) -> Int32 = {
    credits match {
        [head | tail] => { head }
        [] => { 0 }
    }
}

fun compute_invoice: (plan: Plan, usage: Usage) -> Invoice = {
    val Plan { base, per_seat, trial } = plan;
    val Usage { seats, incidents, coupon, credits } = usage;

    val seat_total = seats * per_seat;
    val subtotal = base + seat_total;
    val fallback: () -> Int32 = || 0;
    val discount = (coupon, fallback) coupon_discount;
    val trial_discount = trial |> trial_credit;
    val surcharge = incidents |> incident_surcharge;
    val credit = credits |> first_credit;
    val total = subtotal + surcharge - discount - trial_discount - credit;

    Invoice {
        subtotal: subtotal,
        discount: discount + trial_discount + credit,
        total: total,
        review: total > 500
    }
}

fun main: () -> Int32 = {
    val plan = Plan {
        base: 100,
        per_seat: 12,
        trial: true
    };
    val credits = () default_credits;
    val usage = Usage {
        seats: 18,
        incidents: 1,
        coupon: None,
        credits: credits
    };
    val invoice = (plan, usage) compute_invoice;
    invoice.total
}
