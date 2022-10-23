use std::collections::{BTreeMap, BinaryHeap};

use crate::models::{Order, PartialOrder, Receipt, Side};

#[derive(Default, Debug)]
pub struct MatchingEngine {
    /// The last sequence number
    ///
    /// Orders with a lower sequence number have priority over orders with
    /// a higher sequence number assuming the price is the same.
    pub ordinal: u64,

    /// The "Bid" or "Buy" side of the order book; ordered by ordinal number.
    pub bids: BTreeMap<u64, BinaryHeap<PartialOrder>>,
    /// The "Ask" or "Sell" side of the order book; ordered by ordinal number.
    pub asks: BTreeMap<u64, BinaryHeap<PartialOrder>>,

    /// Previous matches for record keeping
    pub matches: Vec<Receipt>,
}

impl MatchingEngine {
    /// Creates a new [`MatchingEngine`].
    pub fn new() -> Self {
        MatchingEngine {
            ordinal: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            matches: Vec::new(),
        }
    }

    /// Returns the total amount of all the resting orders at a specific
    /// price level of either the Buy or Sell orderbook.
    ///
    /// An amount of 0 indicates that no orders are present at that price
    /// level.
    pub fn get_amount_at_price_level(&self, price: u64, side: Side) -> u64 {
        match side {
            Side::Buy => self.bids.get(&price).map_or_else(
                || 0,
                |price_level| price_level.iter().map(|order| order.amount).sum(),
            ),
            Side::Sell => self.asks.get(&price).map_or_else(
                || 0,
                |price_level| price_level.iter().map(|order| order.amount).sum(),
            ),
        }
    }

    /// Processes an incoming [`Order`] and returns a [`Receipt`].
    ///
    /// This includes matching the order to whatever is in the current books
    /// and adding the remainder (if any) to the book for future matching.
    pub fn process(&mut self, order: Order) -> Receipt {
        // Assign a sequence number to the incoming order
        self.ordinal += 1;
        let ordinal = self.ordinal;

        let original_amount = order.amount;
        let mut partial = order.into_partial_order(ordinal, original_amount);

        // Orders are matched to the opposite side
        let receipt = match &partial.side {
            Side::Buy => {
                // Fetch all sell resting orders that have a maximum price
                // equal to the incoming order limit price
                let orderbook_entries = self.asks.range_mut(u64::MIN..=partial.price);

                let receipt = MatchingEngine::match_order(&partial, orderbook_entries, ordinal);
                let matched_amount = Self::get_matched_amount(&receipt);

                // Add remaining incoming order to the book if it
                // did not fully match
                if matched_amount < original_amount {
                    partial.amount = original_amount - matched_amount;
                    let price = partial.price;
                    let bids = self.bids.entry(price).or_insert(vec![].into());
                    bids.push(partial);
                }
                receipt
            }
            Side::Sell => {
                // Fetch all buy resting orders that have a minimum price
                // equal to the incoming order limit price
                let orderbook_entries = self.bids.range_mut(partial.price..=u64::MAX);

                let receipt = MatchingEngine::match_order(&partial, orderbook_entries, ordinal);
                let matched_amount: u64 = Self::get_matched_amount(&receipt);

                // Add remaining incoming order to the book if it
                // did not fully match
                if matched_amount < original_amount {
                    partial.amount = original_amount - matched_amount;
                    let price = partial.price;
                    let asks = self.asks.entry(price).or_insert(vec![].into());
                    asks.push(partial);
                }
                receipt
            }
        };

        // Cleanup: Remove price entries without orders from the orderbook
        self.asks.retain(|_, orders| !orders.is_empty());
        self.bids.retain(|_, orders| !orders.is_empty());

        // Keep a log of matches
        self.matches.push(receipt.clone());
        receipt
    }

    fn get_matched_amount(receipt: &Receipt) -> u64 {
        receipt.matches.iter().map(|m| m.amount).sum()
    }

    /// Matches an order to the provided order book side.
    ///
    /// # Parameters
    /// - `order`: the order to match to the book
    /// - `orderbook_entries`: a pre-filtered iterator for order book_entry in the correct price range
    /// - `ordinal` the next ordinal number to use if a position is opened
    fn match_order<'a, T>(order: &PartialOrder, mut orderbook_entries: T, ordinal: u64) -> Receipt
    where
        T: Iterator<Item = (&'a u64, &'a mut BinaryHeap<PartialOrder>)>,
    {
        let mut remaining_amount = order.amount;
        let mut matches = vec![];
        let mut self_matches = BinaryHeap::from(vec![]);

        // Try to match an order as long as it still has a remaining amount
        'outer: while remaining_amount > 0 {
            match orderbook_entries.next() {
                Some((price, price_level)) => {
                    // Remove the resting order with the lowest sequence number
                    // from the orderbook entry in order to try to match it
                    while let Some(mut opposite_order) = price_level.pop() {
                        // Check if it's your own order to avoid self-matching; resting
                        // orders that result in a self-match are added back to the orderbook
                        // at the end
                        if order.signer == opposite_order.signer {
                            self_matches.push(opposite_order);
                            continue;
                        }

                        let matched_amount = u64::min(order.amount, opposite_order.amount);
                        remaining_amount -= matched_amount;

                        // If the opposite order has any quantity left it means that the incoming fully matched;
                        // Therefore the remaining of the opposite order is added to the book and there is nothing
                        // left to match with
                        let opposite_order_after_match =
                            PartialOrder::take_from(&mut opposite_order, matched_amount, *price);
                        matches.push(opposite_order.clone());
                        if opposite_order_after_match.remaining > 0 {
                            price_level.push(opposite_order_after_match);
                            break 'outer;
                        }
                    }

                    // Add back self-matches to the price-level
                    price_level.append(&mut self_matches);
                }
                // Nothing left to match with
                None => break 'outer,
            }
        }

        Receipt { ordinal, matches }
    }
}

#[cfg(test)]
mod tests {
    // reduce the warnings for naming tests
    #![allow(non_snake_case)]

    use super::*;

    #[test]
    fn test_MatchingEngine_process_partially_match_order() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Sell,
            signer: "ALICE".to_string(),
        });
        assert_eq!(alice_receipt.matches, vec![]);
        assert_eq!(alice_receipt.ordinal, 1);

        let bob_receipt = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Buy,
            signer: "BOB".to_string(),
        });
        assert_eq!(
            bob_receipt.matches,
            vec![PartialOrder {
                price: 10,
                amount: 1,
                remaining: 0,
                side: Side::Sell,
                signer: "ALICE".to_string(),
                ordinal: 1
            }]
        );
        assert_eq!(bob_receipt.ordinal, 2);

        assert!(matching_engine.asks.is_empty());
        assert_eq!(matching_engine.bids.len(), 1);
        assert_eq!(matching_engine.get_amount_at_price_level(10, Side::Buy), 1);
    }

    #[test]
    fn test_MatchingEngine_process_fully_match_order() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Sell,
            signer: "ALICE".to_string(),
        });
        assert_eq!(alice_receipt.matches, vec![]);
        assert_eq!(alice_receipt.ordinal, 1);

        let bob_receipt = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Buy,
            signer: "BOB".to_string(),
        });

        assert_eq!(
            bob_receipt.matches,
            vec![PartialOrder {
                price: 10,
                amount: 2,
                remaining: 0,
                side: Side::Sell,
                signer: "ALICE".to_string(),
                ordinal: 1
            }]
        );

        // A fully matched order doesn't remain in the book
        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
    }

    #[test]
    fn test_MatchingEngine_process_fully_match_order_multi_match() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Sell,
            signer: "ALICE".to_string(),
        });
        assert_eq!(alice_receipt.matches, vec![]);
        assert_eq!(alice_receipt.ordinal, 1);

        let charlie_receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Sell,
            signer: "CHARLIE".to_string(),
        });
        assert_eq!(charlie_receipt.matches, vec![]);
        assert_eq!(charlie_receipt.ordinal, 2);

        let bob_receipt = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Buy,
            signer: "BOB".to_string(),
        });

        assert_eq!(
            bob_receipt.matches,
            vec![
                PartialOrder {
                    price: 10,
                    amount: 1,
                    remaining: 0,
                    side: Side::Sell,
                    signer: "ALICE".to_string(),
                    ordinal: 1
                },
                PartialOrder {
                    price: 10,
                    amount: 1,
                    remaining: 0,
                    side: Side::Sell,
                    signer: "CHARLIE".to_string(),
                    ordinal: 2
                }
            ]
        );
        // A fully matched order doesn't remain in the book
        assert!(matching_engine.asks.is_empty());
        assert!(matching_engine.bids.is_empty());
    }

    #[test]
    fn test_MatchingEngine_process_fully_match_order_no_self_match() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt_sell = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Sell,
            signer: "ALICE".to_string(),
        });
        assert_eq!(alice_receipt_sell.matches, vec![]);
        assert_eq!(alice_receipt_sell.ordinal, 1);

        let charlie_receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Sell,
            signer: "CHARLIE".to_string(),
        });
        assert_eq!(charlie_receipt.matches, vec![]);
        assert_eq!(charlie_receipt.ordinal, 2);

        let alice_receipt_buy = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Buy,
            signer: "ALICE".to_string(),
        });

        assert_eq!(
            alice_receipt_buy.matches,
            vec![PartialOrder {
                price: 10,
                amount: 1,
                remaining: 0,
                side: Side::Sell,
                signer: "CHARLIE".to_string(),
                ordinal: 2
            }]
        );

        // A fully matched order doesn't remain in the book
        assert_eq!(matching_engine.asks.len(), 1);
        assert_eq!(matching_engine.get_amount_at_price_level(10, Side::Sell), 1);
        assert_eq!(matching_engine.bids.len(), 1);
        assert_eq!(matching_engine.get_amount_at_price_level(10, Side::Buy), 1);
    }

    #[test]
    fn test_MatchingEngine_process_no_match() {
        let mut matching_engine = MatchingEngine::new();

        let alice_receipt = matching_engine.process(Order {
            price: 10,
            amount: 2,
            side: Side::Sell,
            signer: "ALICE".to_string(),
        });
        assert_eq!(alice_receipt.matches, vec![]);
        assert_eq!(alice_receipt.ordinal, 1);

        let bob_receipt = matching_engine.process(Order {
            price: 11,
            amount: 2,
            side: Side::Sell,
            signer: "BOB".to_string(),
        });

        assert_eq!(bob_receipt.matches, vec![]);
        assert_eq!(matching_engine.asks.len(), 2);

        assert_eq!(matching_engine.get_amount_at_price_level(10, Side::Sell), 2);
        assert_eq!(matching_engine.get_amount_at_price_level(11, Side::Sell), 2);
    }

    #[test]
    fn test_MatchingEngine_process_increment_ordinal_matching_engine() {
        let mut matching_engine = MatchingEngine::new();
        assert_eq!(matching_engine.ordinal, 0);
        let receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Buy,
            signer: "ALICE".to_string(),
        });
        assert_eq!(receipt.ordinal, matching_engine.ordinal);

        let receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Buy,
            signer: "BOB".to_string(),
        });
        assert_eq!(receipt.ordinal, matching_engine.ordinal);

        let receipt = matching_engine.process(Order {
            price: 10,
            amount: 1,
            side: Side::Buy,
            signer: "CHARLIE".to_string(),
        });
        assert_eq!(receipt.ordinal, matching_engine.ordinal);
        assert_eq!(matching_engine.ordinal, 3);
    }
}
