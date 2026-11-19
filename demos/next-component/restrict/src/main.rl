import domain.{calculate_subtotal, calculate_member_discount, calculate_shipping}

pub record QuoteResult {
    subtotal: Int32,
    discount: Int32,
    shipping: Int32,
    total: Int32
}

// This is the application boundary exported through the Component Model.
// HTTP, React and Next.js remain outside Restrict.
pub fun quote: (unit_price: Int32, quantity: Int32, member: Boolean) -> QuoteResult = {
    val subtotal = (unit_price, quantity) calculate_subtotal
    val discount = (subtotal, member) calculate_member_discount
    val amount = subtotal - discount
    val shipping = amount calculate_shipping

    QuoteResult {
        subtotal: subtotal,
        discount: discount,
        shipping: shipping,
        total: amount + shipping
    }
}
