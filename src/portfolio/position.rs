use crate::{
    assets::{Asset, MarketEvent, MarketEventDetail, Side},
    portfolio::error::PortfolioError,
    strategy::Decision,
    trading::execution::{FeeAmount, Fees, FillEvent},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use uuid::Uuid;

use super::balance::Balance;

pub type PositionId = String;
pub fn determine_position_id(core_id: Uuid, asset: &Asset) -> PositionId {
    format!("{}_{}_position", core_id, asset)
}
#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Position {
    pub position_id: PositionId,
    pub meta: PositionMeta,
    pub asset: Asset,
    pub side: Side,
    pub quantity: f64,
    pub enter_fees: Fees,
    pub enter_fees_total: FeeAmount,
    pub enter_avg_price_gross: f64,
    pub enter_value_gross: f64,
    pub exit_fees: Fees,
    pub exit_fees_total: FeeAmount,
    pub exit_avg_price_gross: f64,
    pub exit_value_gross: f64,
    pub current_symbol_price: f64,
    pub current_value_gross: f64,
    pub unrealised_profit_loss: f64,
    pub realised_profit_loss: f64,
    pub n_position_updates: i64,
}

impl Position {
    pub fn builder() -> PositionBuilder {
        PositionBuilder::new()
    }
    pub fn calculate_avg_price_gross(fill: &FillEvent) -> f64 {
        (fill.fill_value_gross / fill.quantity).abs()
    }
    pub fn parse_entry_side(fill: &FillEvent) -> Result<Side, PortfolioError> {
        match fill.decision {
            Decision::Long if fill.quantity.is_sign_positive() => Ok(Side::Buy),
            Decision::Short if fill.quantity.is_sign_negative() => Ok(Side::Sell),
            Decision::CloseLong | Decision::CloseShort => {
                Err(PortfolioError::CannotEnterPositionWithExitFill)
            }
            _ => Err(PortfolioError::ParseEntrySide),
        }
    }
    pub fn determine_exit_decision(&self) -> Decision {
        match self.side {
            Side::Buy => Decision::CloseLong,
            Side::Sell => Decision::CloseShort,
        }
    }
    pub fn calculate_unrealised_profit_loss(&self) -> f64 {
        let approx_total_fees = self.enter_fees_total * 2.0;

        match self.side {
            Side::Buy => self.current_value_gross - self.enter_value_gross - approx_total_fees,
            Side::Sell => self.enter_value_gross - self.current_value_gross - approx_total_fees,
        }
    }
    pub fn calculate_realised_profit_loss(&self) -> f64 {
        let total_fees = self.enter_fees_total + self.exit_fees_total;
        match self.side {
            Side::Buy => self.exit_value_gross - self.enter_value_gross - total_fees,
            Side::Sell => self.enter_value_gross - self.exit_value_gross - total_fees,
        }
    }
    pub fn calculate_profit_loss_return(&self) -> f64 {
        self.realised_profit_loss / self.enter_value_gross
    }
    pub fn enter(engine_id: Uuid, fill: &FillEvent) -> Result<Position, PortfolioError> {
        let metadata = PositionMeta {
            enter_time: fill.market_meta.time,
            update_time: fill.time,
            exit_balance: None,
        };
        let enter_fees_total = fill.fees.calculate_total_fees(fill.fill_value_gross);
        let enter_avg_price_gross = Position::calculate_avg_price_gross(fill);
        let unrealised_profit_loss = -enter_fees_total * 2.0;
        Ok(Position {
            position_id: determine_position_id(engine_id, &fill.asset),
            asset: fill.asset.clone(),
            meta: metadata,
            side: Position::parse_entry_side(fill)?,
            quantity: fill.quantity,
            enter_fees: fill.fees,
            enter_fees_total,
            enter_avg_price_gross,
            enter_value_gross: fill.fill_value_gross,
            exit_fees: Fees::default(),
            exit_fees_total: 0.0,
            exit_avg_price_gross: 0.0,
            exit_value_gross: 0.0,
            current_symbol_price: enter_avg_price_gross,
            current_value_gross: fill.fill_value_gross,
            unrealised_profit_loss,
            realised_profit_loss: 0.0,
            n_position_updates: 0,
        })
    }
    pub fn update(&mut self, market: &MarketEvent) -> Option<PositionUpdate> {
        // Determine close from MarketEvent
        let close = match &market.detail {
            MarketEventDetail::Trade(trade) => trade.price,
            MarketEventDetail::Candle(candle) => candle.close,
            MarketEventDetail::BacktestCandle((candle, _)) => candle.close,
            MarketEventDetail::OrderBookL1(book_l1) => book_l1.volume_weighted_mid_price(),
        };
        self.meta.update_time = market.time;
        self.current_symbol_price = close;
        self.current_value_gross = close * self.quantity.abs();
        self.unrealised_profit_loss = self.calculate_unrealised_profit_loss();
        self.n_position_updates = self.n_position_updates + 1;
        Some(PositionUpdate::from(self))
    }
    pub fn exit(
        &mut self,
        mut balance: Balance,
        fill: &FillEvent,
    ) -> Result<PositionExit, PortfolioError> {
        if fill.decision.is_entry() {
            return Err(PortfolioError::CannotExitPositionWithEntryFill);
        }
        self.exit_fees = fill.fees;
        self.exit_fees_total = fill.fees.calculate_total_fees(fill.fill_value_gross);
        self.exit_value_gross = fill.fill_value_gross;
        self.exit_avg_price_gross = Position::calculate_avg_price_gross(fill);
        self.realised_profit_loss = self.calculate_realised_profit_loss();
        self.unrealised_profit_loss = self.realised_profit_loss;
        balance.total += self.realised_profit_loss;
        self.meta.update_time = fill.time;
        self.meta.exit_balance = Some(balance);
        PositionExit::try_from(self)
    }
}

/// Builder to construct [`Position`] instances.
#[derive(Debug, Default)]
pub struct PositionBuilder {
    pub position_id: Option<PositionId>,
    pub asset: Option<Asset>,
    pub meta: Option<PositionMeta>,
    pub side: Option<Side>,
    pub quantity: Option<f64>,
    pub enter_fees: Option<Fees>,
    pub enter_fees_total: Option<FeeAmount>,
    pub enter_avg_price_gross: Option<f64>,
    pub enter_value_gross: Option<f64>,
    pub exit_fees: Option<Fees>,
    pub exit_fees_total: Option<FeeAmount>,
    pub exit_avg_price_gross: Option<f64>,
    pub exit_value_gross: Option<f64>,
    pub current_symbol_price: Option<f64>,
    pub current_value_gross: Option<f64>,
    pub unrealised_profit_loss: Option<f64>,
    pub realised_profit_loss: Option<f64>,
    pub n_position_updates: Option<i64>,
}

impl PositionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn position_id(self, value: PositionId) -> Self {
        Self {
            position_id: Some(value),
            ..self
        }
    }

    pub fn instrument(self, value: Asset) -> Self {
        Self {
            asset: Some(value),
            ..self
        }
    }

    pub fn meta(self, value: PositionMeta) -> Self {
        Self {
            meta: Some(value),
            ..self
        }
    }

    pub fn side(self, value: Side) -> Self {
        Self {
            side: Some(value),
            ..self
        }
    }

    pub fn quantity(self, value: f64) -> Self {
        Self {
            quantity: Some(value),
            ..self
        }
    }

    pub fn enter_fees(self, value: Fees) -> Self {
        Self {
            enter_fees: Some(value),
            ..self
        }
    }

    pub fn enter_fees_total(self, value: FeeAmount) -> Self {
        Self {
            enter_fees_total: Some(value),
            ..self
        }
    }

    pub fn enter_avg_price_gross(self, value: f64) -> Self {
        Self {
            enter_avg_price_gross: Some(value),
            ..self
        }
    }

    pub fn enter_value_gross(self, value: f64) -> Self {
        Self {
            enter_value_gross: Some(value),
            ..self
        }
    }

    pub fn exit_fees(self, value: Fees) -> Self {
        Self {
            exit_fees: Some(value),
            ..self
        }
    }

    pub fn exit_fees_total(self, value: FeeAmount) -> Self {
        Self {
            exit_fees_total: Some(value),
            ..self
        }
    }

    pub fn exit_avg_price_gross(self, value: f64) -> Self {
        Self {
            exit_avg_price_gross: Some(value),
            ..self
        }
    }

    pub fn exit_value_gross(self, value: f64) -> Self {
        Self {
            exit_value_gross: Some(value),
            ..self
        }
    }

    pub fn current_symbol_price(self, value: f64) -> Self {
        Self {
            current_symbol_price: Some(value),
            ..self
        }
    }

    pub fn current_value_gross(self, value: f64) -> Self {
        Self {
            current_value_gross: Some(value),
            ..self
        }
    }

    pub fn unrealised_profit_loss(self, value: f64) -> Self {
        Self {
            unrealised_profit_loss: Some(value),
            ..self
        }
    }

    pub fn realised_profit_loss(self, value: f64) -> Self {
        Self {
            realised_profit_loss: Some(value),
            ..self
        }
    }

    pub fn build(self) -> Result<Position, PortfolioError> {
        Ok(Position {
            position_id: self
                .position_id
                .ok_or(PortfolioError::BuilderIncomplete("position_id"))?,
            asset: self
                .asset
                .ok_or(PortfolioError::BuilderIncomplete("instrument"))?,
            meta: self.meta.ok_or(PortfolioError::BuilderIncomplete("meta"))?,
            side: self.side.ok_or(PortfolioError::BuilderIncomplete("side"))?,
            quantity: self
                .quantity
                .ok_or(PortfolioError::BuilderIncomplete("quantity"))?,
            enter_fees: self
                .enter_fees
                .ok_or(PortfolioError::BuilderIncomplete("enter_fees"))?,
            enter_fees_total: self
                .enter_fees_total
                .ok_or(PortfolioError::BuilderIncomplete("enter_fees_total"))?,
            enter_avg_price_gross: self
                .enter_avg_price_gross
                .ok_or(PortfolioError::BuilderIncomplete("enter_avg_price_gross"))?,
            enter_value_gross: self
                .enter_value_gross
                .ok_or(PortfolioError::BuilderIncomplete("enter_value_gross"))?,
            exit_fees: self
                .exit_fees
                .ok_or(PortfolioError::BuilderIncomplete("exit_fees"))?,
            exit_fees_total: self
                .exit_fees_total
                .ok_or(PortfolioError::BuilderIncomplete("exit_fees_total"))?,
            exit_avg_price_gross: self
                .exit_avg_price_gross
                .ok_or(PortfolioError::BuilderIncomplete("exit_avg_price_gross"))?,
            exit_value_gross: self
                .exit_value_gross
                .ok_or(PortfolioError::BuilderIncomplete("exit_value_gross"))?,
            current_symbol_price: self
                .current_symbol_price
                .ok_or(PortfolioError::BuilderIncomplete("current_symbol_price"))?,
            current_value_gross: self
                .current_value_gross
                .ok_or(PortfolioError::BuilderIncomplete("current_value_gross"))?,
            unrealised_profit_loss: self
                .unrealised_profit_loss
                .ok_or(PortfolioError::BuilderIncomplete("unrealised_profit_loss"))?,
            realised_profit_loss: self
                .realised_profit_loss
                .ok_or(PortfolioError::BuilderIncomplete("realised_profit_loss"))?,
            n_position_updates: 0,
        })
    }
}

/// Metadata detailing the trace UUIDs & timestamps associated with entering, updating & exiting
/// a [`Position`].
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct PositionMeta {
    /// [`FillEvent`] timestamp that triggered the entering of this [`Position`].
    pub enter_time: DateTime<Utc>,
    pub update_time: DateTime<Utc>,
    pub exit_balance: Option<Balance>,
}

impl Default for PositionMeta {
    fn default() -> Self {
        Self {
            enter_time: Utc::now(),
            update_time: Utc::now(),
            exit_balance: None,
        }
    }
}

/// [`Position`] update event. Occurs as a result of receiving new [`MarketEvent`] data.
#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct PositionUpdate {
    pub position_id: String,
    pub update_time: DateTime<Utc>,
    pub current_symbol_price: f64,
    pub current_value_gross: f64,
    pub unrealised_profit_loss: f64,
}

impl From<&mut Position> for PositionUpdate {
    fn from(updated_position: &mut Position) -> Self {
        Self {
            position_id: updated_position.position_id.clone(),
            update_time: updated_position.meta.update_time,
            current_symbol_price: updated_position.current_symbol_price,
            current_value_gross: updated_position.current_value_gross,
            unrealised_profit_loss: updated_position.unrealised_profit_loss,
        }
    }
}

/// [`Position`] exit event. Occurs as a result of a [`FillEvent`] that exits a [`Position`].
#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct PositionExit {
    pub position_id: String,
    pub exit_time: DateTime<Utc>,
    pub exit_balance: Balance,
    pub exit_fees: Fees,
    pub exit_fees_total: FeeAmount,
    pub exit_avg_price_gross: f64,
    pub exit_value_gross: f64,
    pub realised_profit_loss: f64,
}

impl TryFrom<&mut Position> for PositionExit {
    type Error = PortfolioError;

    fn try_from(exited_position: &mut Position) -> Result<Self, Self::Error> {
        Ok(Self {
            position_id: exited_position.position_id.clone(),
            exit_time: exited_position.meta.update_time,
            exit_balance: exited_position
                .meta
                .exit_balance
                .ok_or(PortfolioError::PositionExit)?,
            exit_fees: exited_position.exit_fees,
            exit_fees_total: exited_position.exit_fees_total,
            exit_avg_price_gross: exited_position.exit_avg_price_gross,
            exit_value_gross: exited_position.exit_value_gross,
            realised_profit_loss: exited_position.realised_profit_loss,
        })
    }
}
