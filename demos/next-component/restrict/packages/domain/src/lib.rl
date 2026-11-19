// Pure business rules. This package has no host capability imports, so the
// same source can be included in another Restrict host artifact later.

pub fun calculate_subtotal: (unit_price: Int32, quantity: Int32) -> Int32 = {
    unit_price * quantity
}

pub fun calculate_member_discount: (subtotal: Int32, member: Boolean) -> Int32 = {
    member then {
        subtotal / 10
    } else {
        0
    }
}

pub fun calculate_shipping: (amount: Int32) -> Int32 = {
    amount >= 5000 then {
        0
    } else {
        500
    }
}
