use std::cmp::Reverse;

/// Simplified side of a position as well as order.
#[derive(Clone, Copy, PartialOrd, PartialEq, Eq, Debug, Ord)]
pub enum Side {
    /// Want to buy
    Buy,
    /// Want to sell
    Sell,
}

impl Default for Side {
    fn default() -> Self {
        Self::Buy
    }
}

/// An order to buy or sell an amount at a given price.
#[derive(Clone, PartialEq, Eq)]
pub struct Order {
    /// Max/min price (depending on the side)
    pub price: u64,
    /// Number of units to trade
    pub amount: u64,
    /// The side of the order (buy or sell)
    ///
    /// Incoming [`Order`]s are matched against the opposite side
    pub side: Side,
    /// The account signer
    pub signer: String,
}

impl Order {
    /// Convert an [`Order`] into a [`PartialOrder`] with the added parameters.
    pub fn into_partial_order(self, ordinal: u64, remaining: u64) -> PartialOrder {
        let Order {
            price,
            amount,
            side,
            signer,
        } = self;
        PartialOrder {
            price,
            amount,
            remaining,
            side,
            signer,
            ordinal,
        }
    }
}

/// An unfilled order that is kept in the system for later filling.
#[derive(Clone, PartialEq, Debug, Eq, Ord, Default)]
pub struct PartialOrder {
    /// Price per unit
    pub price: u64,
    /// Initial number of units in the order
    pub amount: u64,
    /// Remaining number of units after potential matches
    pub remaining: u64,
    /// Buy or sell side of the book
    pub side: Side,
    /// Signer of the order
    pub signer: String,
    /// Sequence number
    pub ordinal: u64,
}

impl PartialOrd for PartialOrder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // this reverses the comparison to create a min heap;
        // therefore, `pop()`ing from a `BinaryHeap` returns the item
        // with the lowest value for `ordinal`
        Reverse(self.ordinal).partial_cmp(&Reverse(other.ordinal))
    }
}

/// A receipt issued to the caller for sending an [`Order`].
#[derive(Clone, PartialOrd, PartialEq, Eq, Debug)]
pub struct Receipt {
    /// Sequence number
    pub ordinal: u64,

    /// Matches that happened immediately
    pub matches: Vec<PartialOrder>,
}

impl PartialOrder {
    /// Splits one [`PartialOrder`] into two by taking a defined `take` amount
    pub fn take_from(pos: &mut PartialOrder, take: u64, price: u64) -> PartialOrder {
        let remaining_amount = pos.remaining - take;
        pos.remaining -= take;

        let mut new = pos.clone();
        new.amount = remaining_amount;
        new.price = price;
        new
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BinaryHeap;

    use super::PartialOrder;

    #[test]
    fn binary_heap_pops_partial_orders_with_smaller_ordinal_first() {
        // Arrange
        let orders = (1..=2)
            .map(|ordinal| PartialOrder {
                ordinal,
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let mut heap = BinaryHeap::from(orders);

        // Act
        let first_order = heap.pop();
        let second_order = heap.pop();

        // Assert
        assert_eq!(first_order.unwrap().ordinal, 1);
        assert_eq!(second_order.unwrap().ordinal, 2);
    }
}
