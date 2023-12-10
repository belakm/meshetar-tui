use super::OrderEvent;

pub struct RiskEvaluator {}
impl RiskEvaluator {
    pub fn evaluate_order(&self, order: OrderEvent) -> Option<OrderEvent> {
        if self.risk_too_high(&order) {
            return None;
        }
        Some(order)
    }
    fn risk_too_high(&self, _: &OrderEvent) -> bool {
        false
    }
}
