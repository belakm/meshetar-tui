use super::error::TraderError;
use crate::{
    assets::{Asset, MarketMeta},
    portfolio::OrderEvent,
    strategy::Decision,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub struct Execution {
    exchange_fee: f64,
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default, Deserialize, Serialize)]
pub struct Fees {
    pub exchange: FeeAmount,
    pub slippage: FeeAmount,
}

impl Fees {
    pub fn calculate_total_fees(&self, gross: f64) -> f64 {
        (self.exchange * gross) + self.slippage
    }
}

/// Communicative type alias for Fee amount as f64.
pub type FeeAmount = f64;

impl Execution {
    pub fn new(exchange_fee: f64) -> Self {
        Execution { exchange_fee }
    }
    pub fn generate_fill(
        &self,
        order: &OrderEvent,
        live_time: bool,
    ) -> Result<FillEvent, TraderError> {
        let fill_time = if live_time { Utc::now() } else { order.time };
        let fill_event = FillEvent::builder()
            .time(fill_time)
            .asset(order.asset.clone())
            .market_meta(order.market_meta)
            .decision(order.decision)
            .quantity(order.quantity)
            .fill_value_gross(order.quantity.abs() * order.market_meta.close)
            .fees(Fees {
                exchange: self.exchange_fee,
                slippage: 0.0,
            })
            .build()?;
        Ok(fill_event)
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct FillEvent {
    pub time: DateTime<Utc>,
    pub asset: Asset,
    pub market_meta: MarketMeta,
    pub decision: Decision,
    pub quantity: f64,
    pub fill_value_gross: f64,
    pub fees: Fees,
}

impl FillEvent {
    /// Returns a [`FillEventBuilder`] instance.
    pub fn builder() -> FillEventBuilder {
        FillEventBuilder::new()
    }
}

#[derive(Debug, Default)]
pub struct FillEventBuilder {
    pub time: Option<DateTime<Utc>>,
    pub asset: Option<Asset>,
    pub decision: Option<Decision>,
    pub quantity: Option<f64>,
    pub fill_value_gross: Option<f64>,
    pub fees: Option<Fees>,
    pub market_meta: Option<MarketMeta>,
}

impl FillEventBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn time(self, value: DateTime<Utc>) -> Self {
        Self {
            time: Some(value),
            ..self
        }
    }

    pub fn asset(self, value: Asset) -> Self {
        Self {
            asset: Some(value),
            ..self
        }
    }

    pub fn decision(self, value: Decision) -> Self {
        Self {
            decision: Some(value),
            ..self
        }
    }

    pub fn quantity(self, value: f64) -> Self {
        Self {
            quantity: Some(value),
            ..self
        }
    }

    pub fn fill_value_gross(self, value: f64) -> Self {
        Self {
            fill_value_gross: Some(value),
            ..self
        }
    }

    pub fn fees(self, value: Fees) -> Self {
        Self {
            fees: Some(value),
            ..self
        }
    }

    pub fn market_meta(self, value: MarketMeta) -> Self {
        Self {
            market_meta: Some(value),
            ..self
        }
    }

    pub fn build(self) -> Result<FillEvent, TraderError> {
        Ok(FillEvent {
            time: self
                .time
                .ok_or(TraderError::FillBuilderIncomplete("time"))?,
            asset: self
                .asset
                .ok_or(TraderError::FillBuilderIncomplete("asset"))?,
            decision: self
                .decision
                .ok_or(TraderError::FillBuilderIncomplete("decision"))?,
            quantity: self
                .quantity
                .ok_or(TraderError::FillBuilderIncomplete("quantity"))?,
            fill_value_gross: self
                .fill_value_gross
                .ok_or(TraderError::FillBuilderIncomplete("fill_value_gross"))?,
            fees: self
                .fees
                .ok_or(TraderError::FillBuilderIncomplete("fees"))?,
            market_meta: self
                .market_meta
                .ok_or(TraderError::FillBuilderIncomplete("market_meta"))?,
        })
    }
}
