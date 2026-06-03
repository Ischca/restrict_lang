// Inventory reorder example.
// Dogfoods primitive field reads from one affine record, Result flow, and OSV calls.

record StockItem {
    on_hand: Int32,
    reserved: Int32,
    reorder_point: Int32,
    lead_days: Int32
}

fun validate_item: (item: StockItem) -> Result<StockItem, Int32> = {
    item.reserved <= item.on_hand then {
        Ok(item)
    } else {
        Err(422)
    }
}

fun reorder_delay: (item: StockItem) -> Int32 = {
    val available = item.on_hand - item.reserved;

    available < item.reorder_point then {
        item.lead_days + 1
    } else {
        0
    }
}

fun plan_item: (item: StockItem) -> Int32 = {
    val checked = item |> validate_item;

    checked match {
        Ok(valid_item) => {
            valid_item |> reorder_delay
        }
        Err(code) => {
            code
        }
    }
}

fun main: () -> Int32 = {
    val item = StockItem {
        on_hand: 12,
        reserved: 7,
        reorder_point: 8,
        lead_days: 3
    };

    item |> plan_item
}
