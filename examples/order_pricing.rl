// Order pricing example.
// Exercises OSV calls, inferred local types, and Boolean matching.

fun clamp_discount: (discount: Int32, subtotal: Int32) -> Int32 = {
    discount > subtotal then {
        subtotal
    } else {
        discount
    }
}

fun expedited_fee: (expedited: Boolean) -> Int32 = {
    expedited match {
        true => { 15 }
        false => { 5 }
    }
}

fun total_due: (subtotal: Int32, coupon: Int32, expedited: Boolean) -> Int32 = {
    val discount = (coupon, subtotal) clamp_discount;
    val shipping = expedited |> expedited_fee;
    val after_discount = subtotal - discount;
    after_discount + shipping
}

fun main: () -> Int32 = {
    (85, 20, true) total_due
}
