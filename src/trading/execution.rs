use super::error::TraderError;
use crate::{
  assets::{MarketMeta, Pair, Side},
  exchange::{
    binance_client::{self, BinanceClient},
    execution::fill_order,
  },
  portfolio::OrderEvent,
  strategy::Decision,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub struct Execution {
  exchange_fee: f64,
  binance_client: BinanceClient,
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

pub type FeeAmount = f64;

impl Execution {
  pub fn new(exchange_fee: f64, binance_client: BinanceClient) -> Self {
    Execution { exchange_fee, binance_client }
  }
  pub async fn generate_fill(
    &self,
    order: &OrderEvent,
    is_live_run: bool,
  ) -> Result<FillEvent, TraderError> {
    log::info!("Received a new order to fill: {:?}", order);

    let fill_time = if is_live_run { Utc::now() } else { order.time };

    let side = if order.decision.is_entry() { Side::Buy } else { Side::Sell };
    let exchange_execution =
      fill_order(&self.binance_client, order.pair.clone(), order.quantity, side)?;

    let fill_event = FillEvent::builder()
      .time(exchange_execution.updated_at)
      .asset(order.pair.clone())
      .market_meta(order.market_meta)
      .decision(order.decision)
      .quantity(exchange_execution.qty)
      .fill_value_gross(exchange_execution.qty.abs() * exchange_execution.price)
      .fees(Fees { exchange: self.exchange_fee, slippage: 0.0 })
      .build()?;
    Ok(fill_event)
  }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct FillEvent {
  pub time: DateTime<Utc>,
  pub asset: Pair,
  pub market_meta: MarketMeta,
  pub decision: Decision,
  pub quantity: f64,
  pub fill_value_gross: f64,
  pub fees: Fees,
}

impl FillEvent {
  pub fn builder() -> FillEventBuilder {
    FillEventBuilder::new()
  }
}

#[derive(Debug, Default)]
pub struct FillEventBuilder {
  pub time: Option<DateTime<Utc>>,
  pub asset: Option<Pair>,
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
    Self { time: Some(value), ..self }
  }

  pub fn asset(self, value: Pair) -> Self {
    Self { asset: Some(value), ..self }
  }

  pub fn decision(self, value: Decision) -> Self {
    Self { decision: Some(value), ..self }
  }

  pub fn quantity(self, value: f64) -> Self {
    Self { quantity: Some(value), ..self }
  }

  pub fn fill_value_gross(self, value: f64) -> Self {
    Self { fill_value_gross: Some(value), ..self }
  }

  pub fn fees(self, value: Fees) -> Self {
    Self { fees: Some(value), ..self }
  }

  pub fn market_meta(self, value: MarketMeta) -> Self {
    Self { market_meta: Some(value), ..self }
  }

  pub fn build(self) -> Result<FillEvent, TraderError> {
    Ok(FillEvent {
      time: self.time.ok_or(TraderError::FillBuilderIncomplete("time"))?,
      asset: self.asset.ok_or(TraderError::FillBuilderIncomplete("asset"))?,
      decision: self.decision.ok_or(TraderError::FillBuilderIncomplete("decision"))?,
      quantity: self.quantity.ok_or(TraderError::FillBuilderIncomplete("quantity"))?,
      fill_value_gross: self
        .fill_value_gross
        .ok_or(TraderError::FillBuilderIncomplete("fill_value_gross"))?,
      fees: self.fees.ok_or(TraderError::FillBuilderIncomplete("fees"))?,
      market_meta: self
        .market_meta
        .ok_or(TraderError::FillBuilderIncomplete("market_meta"))?,
    })
  }
}
