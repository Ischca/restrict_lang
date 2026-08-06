fun validate_stock: (requested: Int32, available: Int32) -> Result<Int32, Int32> = {
    requested <= available then {
        (requested) Result::Ok
    } else {
        (409) Result::Err
    }
}

fun authorize_payment: (amount: Int32, approved: Boolean) -> Result<Int32, Int32> = {
    approved then {
        (amount) Result::Ok
    } else {
        (402) Result::Err
    }
}

fun fulfillment_decision: (
    requested: Int32,
    available: Int32,
    amount: Int32,
    approved: Boolean
) -> Int32 = {
    val stock = (requested, available) validate_stock;

    stock match {
        Ok(quantity) => {
            val payment = (amount, approved) authorize_payment;

            payment match {
                Ok(total) => {
                    quantity + total
                }
                Err(code) => {
                    code
                }
            }
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    (3, 5, 120, true) fulfillment_decision
}
