pub mod account;
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
    assets::{Asset, MarketEvent, MarketMeta, Side},
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
    pub asset: Asset,
    pub decision: Decision,
    pub market_meta: MarketMeta,
    pub quantity: f64,
}

pub struct Portfolio {
    database: Arc<Mutex<Database>>,
    core_id: Uuid,
    allocation_manager: Allocator,
    risk_manager: RiskEvaluator,
    assets: Vec<Asset>,
    statistic_config: StatisticConfig,
}

impl Portfolio {
    pub fn builder() -> PortfolioBuilder {
        PortfolioBuilder::new()
    }
    pub async fn bootstrap_database(&self, starting_cash: f64) -> Result<(), PortfolioError> {
        self.database.lock().await.set_balance(
            self.core_id,
            Balance {
                time: Utc::now(),
                total: starting_cash,
                available: starting_cash,
            },
        )?;
        Ok(())
    }

    pub async fn open_positions(&self) -> Result<Vec<Position>, PortfolioError> {
        let mut database = self.database.lock().await;
        let positions = database.get_all_open_positions(self.core_id)?;
        Ok(positions)
    }

    pub async fn generate_order(
        &mut self,
        signal: &Signal,
        time_is_live: bool,
    ) -> Result<Option<OrderEvent>, PortfolioError> {
        // Determine the position_id & associated Option<Position> related to input SignalEvent
        let position_id = determine_position_id(self.core_id, &signal.asset);
        let position = { self.database.lock().await.get_open_position(&position_id)? };
        // If signal is advising to open a new Position rather than close one, check we have cash
        if position.is_none() && self.no_cash_to_enter_new_position().await? {
            info!("No cash available to open a new position.");
            return Ok(None);
        }
        // Parse signals from Strategy to determine net signal decision & associated strength
        let position = position.as_ref();
        let (signal_decision, signal_strength) =
            match parse_signal_decisions(&position, &signal.signals) {
                None => return Ok(None),
                Some(net_signal) => net_signal,
            };
        // Construct mutable OrderEvent that can be modified by Allocation & Risk management
        let order_time = if time_is_live {
            Utc::now()
        } else {
            signal.time
        };
        let mut order = OrderEvent {
            time: order_time,
            asset: signal.asset.clone(),
            market_meta: signal.market_meta,
            decision: *signal_decision,
            quantity: 1.0,
        };

        let max_value = self
            .database
            .lock()
            .await
            .get_balance(self.core_id)
            .unwrap()
            .available;

        // Manage OrderEvent size allocation
        self.allocation_manager
            .allocate_order(&mut order, position, *signal_strength, max_value);
        // Manage global risk when evaluating OrderEvent - keep the same, refine or cancel
        Ok(self.risk_manager.evaluate_order(order))
    }
    async fn no_cash_to_enter_new_position(&mut self) -> Result<bool, PortfolioError> {
        let res = self
            .database
            .lock()
            .await
            .get_balance(self.core_id)
            .map(|balance| Ok(balance.available == 0.0))
            .map_err(PortfolioError::RepositoryInteraction)?;
        res
    }
    pub async fn generate_exit_order(
        &mut self,
        signal: SignalForceExit,
        live_trading: bool,
    ) -> Result<Option<OrderEvent>, PortfolioError> {
        // Determine PositionId associated with the SignalForceExit
        let position_id = determine_position_id(self.core_id, &signal.asset);

        // Retrieve Option<Position> associated with the PositionId
        let position = match self.database.lock().await.get_open_position(&position_id)? {
            None => {
                info!(
                    position_id = &*position_id,
                    outcome = "no forced exit OrderEvent generated",
                    "cannot generate forced exit OrderEvent for a Position that isn't open"
                );
                return Ok(None);
            }
            Some(position) => position,
        };
        let time = if live_trading {
            Utc::now()
        } else {
            signal.time
        };
        Ok(Some(OrderEvent {
            time,
            asset: signal.asset,
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
        market: MarketEvent,
    ) -> Result<Option<PositionUpdate>, PortfolioError> {
        // Determine the position_id associated to the input MarketEvent
        let position_id = determine_position_id(self.core_id, &market.asset);
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
        fill: &FillEvent,
    ) -> Result<Vec<Event>, PortfolioError> {
        let mut generated_events: Vec<Event> = Vec::with_capacity(2);
        let mut database = self.database.lock().await;
        let mut balance = database.get_balance(self.core_id)?;
        let position_id = determine_position_id(self.core_id, &fill.asset);
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
                let mut stats = database.get_statistics(&asset)?;
                stats.update(&position);

                // Persist exited Position & Updated Market statistics in Repository
                database.set_statistics(asset.clone(), stats)?;
                database.set_exited_position(self.core_id, position)?;
            }
            None => {
                let position = Position::enter(self.core_id, fill)?;
                generated_events.push(Event::PositionNew(position.clone()));
                balance.available += -position.enter_value_gross - position.enter_fees_total;
                database.set_open_position(position)?;
            }
        };
        generated_events.push(Event::Balance(balance));
        database.set_balance(self.core_id, balance)?;
        Ok(generated_events)
    }

    pub async fn get_statistics(&mut self, asset: &Asset) -> Result<TradingSummary, DatabaseError> {
        self.database.lock().await.get_statistics(asset)
    }

    pub async fn reset_statistics_with_time(
        &mut self,
        asset: &Asset,
        starting_time: DateTime<Utc>,
    ) -> Result<(), PortfolioError> {
        let mut database = self.database.lock().await;
        for asset in self.assets.clone() {
            database
                .set_statistics(
                    asset,
                    TradingSummary::init(self.statistic_config, Some(starting_time)),
                )
                .map_err(PortfolioError::RepositoryInteraction)?;
        }
        Ok(())
    }

    pub async fn init_statistics(
        &self,
        statistic_config: StatisticConfig,
    ) -> Result<(), PortfolioError> {
        let mut database = self.database.lock().await;
        for asset in self.assets.clone() {
            database
                .set_statistics(asset, TradingSummary::init(statistic_config, None))
                .map_err(PortfolioError::RepositoryInteraction)?;
        }
        Ok(())
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
    core_id: Option<Uuid>,
    allocation_manager: Option<Allocator>,
    risk_manager: Option<RiskEvaluator>,
    starting_cash: Option<f64>,
    statistic_config: Option<StatisticConfig>,
    assets: Option<Vec<Asset>>,
}

impl PortfolioBuilder {
    pub fn new() -> Self {
        PortfolioBuilder {
            database: None,
            core_id: None,
            allocation_manager: None,
            risk_manager: None,
            starting_cash: None,
            statistic_config: None,
            assets: None,
        }
    }
    pub fn database(self, database: Arc<Mutex<Database>>) -> Self {
        Self {
            database: Some(database),
            ..self
        }
    }
    pub fn core_id(self, value: Uuid) -> Self {
        Self {
            core_id: Some(value),
            ..self
        }
    }
    pub fn allocation_manager(self, value: Allocator) -> Self {
        Self {
            allocation_manager: Some(value),
            ..self
        }
    }
    pub fn risk_manager(self, value: RiskEvaluator) -> Self {
        Self {
            risk_manager: Some(value),
            ..self
        }
    }
    pub fn starting_cash(self, value: f64) -> Self {
        Self {
            starting_cash: Some(value),
            ..self
        }
    }
    pub fn statistic_config(self, value: StatisticConfig) -> Self {
        Self {
            statistic_config: Some(value),
            ..self
        }
    }
    pub fn assets(self, value: Vec<Asset>) -> Self {
        Self {
            assets: Some(value),
            ..self
        }
    }
    pub async fn build(self) -> Result<Portfolio, PortfolioError> {
        let portfolio = Portfolio {
            core_id: self
                .core_id
                .ok_or(PortfolioError::BuilderIncomplete("core_id"))?,
            allocation_manager: self
                .allocation_manager
                .ok_or(PortfolioError::BuilderIncomplete("allocation_manager"))?,
            risk_manager: self
                .risk_manager
                .ok_or(PortfolioError::BuilderIncomplete("risk_manager"))?,
            assets: self
                .assets
                .ok_or(PortfolioError::BuilderIncomplete("assets"))?,
            database: self
                .database
                .ok_or(PortfolioError::BuilderIncomplete("database"))?,
            statistic_config: self
                .statistic_config
                .ok_or(PortfolioError::BuilderIncomplete("statistic_config"))?,
        };

        portfolio
            .bootstrap_database(
                self.starting_cash
                    .ok_or(PortfolioError::BuilderIncomplete("starting_cash"))?,
            )
            .await?;

        Ok(portfolio)
    }
}
