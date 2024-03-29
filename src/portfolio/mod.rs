pub mod allocator;
pub mod balance;
pub mod error;
pub mod position;
pub mod risk;

use self::{
  allocator::Allocator,
  balance::Balance,
  error::PortfolioError,
  position::{determine_position_id, Position, PositionUpdate},
  risk::RiskEvaluator,
};
use crate::{
  assets::{MarketEvent, MarketMeta, Pair, Side},
  database::{error::DatabaseError, Database},
  events::Event,
  statistic::{StatisticConfig, TradingSummary},
  strategy::{Decision, Signal, SignalStrength},
  trading::{execution::FillEvent, SignalForceExit},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct OrderEvent {
  pub time: DateTime<Utc>,
  pub pair: Pair,
  pub decision: Decision,
  pub market_meta: MarketMeta,
  pub quantity: f64,
}

pub struct Portfolio {
  database: Arc<Mutex<Database>>,
  allocation_manager: Allocator,
  risk_manager: RiskEvaluator,
  statistic_config: StatisticConfig,
}

impl Portfolio {
  pub fn builder() -> PortfolioBuilder {
    PortfolioBuilder::new()
  }

  pub async fn open_positions(
    &self,
    core_id: Uuid,
  ) -> Result<Vec<Position>, PortfolioError> {
    let mut database = self.database.lock().await;
    let positions = database.get_all_open_positions(core_id)?;
    Ok(positions)
  }

  pub async fn generate_order(
    &mut self,
    core_id: Uuid,
    signal: &Signal,
    time_is_live: bool,
  ) -> Result<Option<OrderEvent>, PortfolioError> {
    let position_id = determine_position_id(&core_id, &signal.pair);
    let position = { self.database.lock().await.get_open_position(&position_id)? };
    if position.is_none() && self.no_cash_to_enter_new_position(core_id).await? {
      info!("No cash available to open a new position.");
      return Ok(None);
    }
    let position = position.as_ref();
    let (signal_decision, signal_strength) =
      match parse_signal_decisions(&position, &signal.signals) {
        None => return Ok(None),
        Some(net_signal) => net_signal,
      };
    let order_time = if time_is_live { Utc::now() } else { signal.time };
    let mut order = OrderEvent {
      time: order_time,
      pair: signal.pair.clone(),
      market_meta: signal.market_meta,
      decision: *signal_decision,
      quantity: 1.0,
    };
    let max_value =
      { self.database.lock().await.get_balance(core_id).unwrap().available };
    self.allocation_manager.allocate_order(
      &mut order,
      position,
      *signal_strength,
      max_value,
    );
    log::info!("ORDER {:?}", order);
    Ok(self.risk_manager.evaluate_order(order))
  }
  async fn no_cash_to_enter_new_position(
    &mut self,
    core_id: Uuid,
  ) -> Result<bool, PortfolioError> {
    let res = self
      .database
      .lock()
      .await
      .get_balance(core_id)
      .map(|balance| Ok(balance.available == 0.0))
      .map_err(PortfolioError::RepositoryInteraction)?;
    res
  }
  pub async fn generate_exit_order(
    &mut self,
    core_id: Uuid,
    signal: SignalForceExit,
    live_trading: bool,
  ) -> Result<Option<OrderEvent>, PortfolioError> {
    // Determine PositionId associated with the SignalForceExit
    let position_id = determine_position_id(&core_id, &signal.asset);

    // Retrieve Option<Position> associated with the PositionId
    let position = match self.database.lock().await.get_open_position(&position_id)? {
      None => {
        info!(
          position_id = &*position_id,
          outcome = "no forced exit OrderEvent generated",
          "cannot generate forced exit OrderEvent for a Position that isn't open"
        );
        return Ok(None);
      },
      Some(position) => position,
    };
    let time = if live_trading { Utc::now() } else { signal.time };
    Ok(Some(OrderEvent {
      time,
      pair: signal.asset,
      market_meta: MarketMeta {
        close: position.current_symbol_price,
        time: position.meta.update_time,
      },
      decision: position.determine_exit_decision(),
      quantity: 0.0 - position.quantity,
    }))
  }

  pub async fn update_from_market(
    &mut self,
    core_id: Uuid,
    market: MarketEvent,
  ) -> Result<Option<PositionUpdate>, PortfolioError> {
    // Determine the position_id associated to the input MarketEvent
    let position_id = determine_position_id(&core_id, &market.pair);
    let mut database = self.database.lock().await;
    // Update Position if Portfolio has an open Position for that Symbol-Exchange combination
    if let Some(mut position) = database.get_open_position(&position_id)? {
      // Derive PositionUpdate event that communicates the open Position's change in state
      if let Some(position_update) = position.update(&market) {
        // Save updated open Position in the repository
        database.set_open_position(position)?;
        return Ok(Some(position_update));
      }
    }

    Ok(None)
  }

  pub async fn update_from_fill(
    &mut self,
    core_id: Uuid,
    fill: &FillEvent,
  ) -> Result<Vec<Event>, PortfolioError> {
    let mut generated_events: Vec<Event> = Vec::with_capacity(2);
    let mut database = self.database.lock().await;
    let mut balance = database.get_balance(core_id)?;
    let position_id = determine_position_id(&core_id, &fill.asset);
    balance.time = fill.time;
    match database.remove_position(&position_id)? {
      Some(mut position) => {
        let position_exit = position.exit(balance, fill)?;
        generated_events.push(Event::PositionExit(position_exit));

        balance.available += position.enter_value_gross
          + position.realised_profit_loss
          + position.enter_fees_total;
        balance.total += position.realised_profit_loss;

        let asset = position.asset.clone();
        let mut stats = database.get_statistics(&core_id)?;
        stats.update(&position);

        // Persist exited Position & Updated Market statistics in Repository
        database.set_statistics(core_id, stats)?;
        database.set_exited_position(core_id, position)?;
      },
      None => {
        let position = Position::enter(core_id, fill)?;
        generated_events.push(Event::PositionNew(position.clone()));
        balance.available += -position.enter_value_gross - position.enter_fees_total;
        database.set_open_position(position)?;
      },
    };
    generated_events.push(Event::Balance(balance));
    database.set_balance(core_id, balance)?;
    Ok(generated_events)
  }

  pub async fn get_statistics(
    &mut self,
    core_id: &Uuid,
  ) -> Result<TradingSummary, DatabaseError> {
    self.database.lock().await.get_statistics(core_id)
  }
}

fn parse_signal_decisions<'a>(
  position: &'a Option<&Position>,
  signals: &'a HashMap<Decision, SignalStrength>,
) -> Option<(&'a Decision, &'a SignalStrength)> {
  let signal_close_long = signals.get_key_value(&Decision::CloseLong);
  let signal_long = signals.get_key_value(&Decision::Long);
  let signal_close_short = signals.get_key_value(&Decision::CloseShort);
  let signal_short = signals.get_key_value(&Decision::Short);

  // If an existing Position exists, check for net close signals
  if let Some(position) = position {
    return match position.side {
      Side::Buy if signal_close_long.is_some() => signal_close_long,
      Side::Sell if signal_close_short.is_some() => signal_close_short,
      _ => None,
    };
  }

  // Else check for net open signals
  match (signal_long, signal_short) {
    (Some(signal_long), None) => Some(signal_long),
    (None, Some(signal_short)) => Some(signal_short),
    _ => None,
  }
}

pub struct PortfolioBuilder {
  database: Option<Arc<Mutex<Database>>>,
  allocation_manager: Option<Allocator>,
  risk_manager: Option<RiskEvaluator>,
  statistic_config: Option<StatisticConfig>,
}

impl PortfolioBuilder {
  pub fn new() -> Self {
    PortfolioBuilder {
      database: None,
      allocation_manager: None,
      risk_manager: None,
      statistic_config: None,
    }
  }
  pub fn database(self, database: Arc<Mutex<Database>>) -> Self {
    Self { database: Some(database), ..self }
  }
  pub fn allocation_manager(self, value: Allocator) -> Self {
    Self { allocation_manager: Some(value), ..self }
  }
  pub fn risk_manager(self, value: RiskEvaluator) -> Self {
    Self { risk_manager: Some(value), ..self }
  }
  pub fn statistic_config(self, value: StatisticConfig) -> Self {
    Self { statistic_config: Some(value), ..self }
  }
  pub async fn build(self) -> Result<Portfolio, PortfolioError> {
    let portfolio = Portfolio {
      allocation_manager: self
        .allocation_manager
        .ok_or(PortfolioError::BuilderIncomplete("allocation_manager"))?,
      risk_manager: self
        .risk_manager
        .ok_or(PortfolioError::BuilderIncomplete("risk_manager"))?,
      database: self.database.ok_or(PortfolioError::BuilderIncomplete("database"))?,
      statistic_config: self
        .statistic_config
        .ok_or(PortfolioError::BuilderIncomplete("statistic_config"))?,
    };

    Ok(portfolio)
  }
}
