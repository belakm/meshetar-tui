use crate::strategy::{Decision, SignalStrength};

use super::{position::Position, OrderEvent};

pub struct Allocator {
    pub default_order_value: f64,
}

impl Allocator {
    pub fn allocate_order(
        &self,
        order: &mut OrderEvent,
        position: Option<&Position>,
        signal_strength: SignalStrength,
        max_value: f64,
    ) {
        // Calculate exact order_size, then round it to a more appropriate decimal place
        // let default_order_size = self.default_order_value / order.market_meta.close;
        let default_order_size = max_value / order.market_meta.close;
        let default_order_size = (default_order_size * 10000000.0).floor() / 10000000.0;

        match order.decision {
            // Entry
            Decision::Long => order.quantity = default_order_size * signal_strength.0,

            // Entry
            Decision::Short => order.quantity = -default_order_size * signal_strength.0,

            // Exit
            _ => order.quantity = 0.0 - position.as_ref().unwrap().quantity,
        }
    }
}
