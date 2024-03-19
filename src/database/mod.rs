pub mod error;
pub mod sqlite;

use self::{error::DatabaseError, sqlite::DB_POOL};
use crate::{
  assets::{
    asset_ticker::{self, KlineDetail},
    error::AssetError,
    Candle, MarketEvent, MarketEventDetail, Pair,
  },
  components::list::LabelValueItem,
  events::Event,
  exchange::{
    binance_client::{self, BinanceClient},
    ExchangeAccount,
  },
  portfolio::{
    account::{new_account_stream, Account},
    balance::{Balance, BalanceId},
    position::{determine_position_id, Position, PositionId},
  },
  statistic::TradingSummary,
  utils::formatting::duration_to_readable,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use tokio::sync::{
  mpsc::{self, Receiver, Sender},
  Mutex,
};
use uuid::Uuid;

pub struct Database {
  open_positions: HashMap<PositionId, Position>,
  closed_positions: HashMap<String, Vec<Position>>,
  current_balances: HashMap<BalanceId, Balance>,
  exchange_balances: HashMap<String, Balance>,
  statistics: HashMap<Uuid, TradingSummary>,
  exchange_account: ExchangeAccount,
  asset_prices: HashMap<String, KlineDetail>,
  event_tx: mpsc::UnboundedSender<Event>,
  stream_url: String,
}
impl Database {
  pub async fn new(
    event_tx: mpsc::UnboundedSender<Event>,
    stream_url: String,
  ) -> Result<Database, DatabaseError> {
    sqlite::initialize().await?;

    let database = Database {
      open_positions: HashMap::new(),
      closed_positions: HashMap::new(),
      current_balances: HashMap::new(),
      exchange_balances: HashMap::new(),
      statistics: HashMap::new(),
      exchange_account: ExchangeAccount::default(),
      asset_prices: HashMap::new(),
      event_tx,
      stream_url,
    };

    Ok(database)
  }

  pub fn set_balance(
    &mut self,
    core_id: Uuid,
    balance: Balance,
  ) -> Result<(), DatabaseError> {
    self.current_balances.insert(Balance::balance_id(core_id), balance);
    Ok(())
  }

  pub fn get_balance(&mut self, core_id: Uuid) -> Result<Balance, DatabaseError> {
    self.current_balances.get(&Balance::balance_id(core_id)).copied().ok_or(
      DatabaseError::DataMissing(format!(
        "Balance for {} missing on database lookup.",
        core_id
      )),
    )
  }

  pub fn set_exchange_balances(&mut self, exchange_balances: Vec<(String, Balance)>) {
    for (assetName, balance) in exchange_balances {
      self.exchange_balances.insert(assetName, balance);
    }
  }

  pub fn get_exchange_balances(&self) -> HashMap<String, Balance> {
    self.exchange_balances.clone()
  }

  pub fn get_exchange_account(&self) -> ExchangeAccount {
    self.exchange_account.clone()
  }

  pub fn set_exchange_account(&mut self, value: ExchangeAccount) {
    self.exchange_account = value
  }

  pub fn set_open_position(&mut self, position: Position) -> Result<(), DatabaseError> {
    self.open_positions.insert(position.position_id.clone(), position);
    Ok(())
  }

  pub fn get_open_position(
    &mut self,
    position_id: &PositionId,
  ) -> Result<Option<Position>, DatabaseError> {
    Ok(self.open_positions.get(position_id).map(Position::clone))
  }

  pub fn get_open_positions(
    &mut self,
    core_id: &Uuid,
    pairs: Vec<Pair>,
  ) -> Result<Vec<Position>, DatabaseError> {
    Ok(
      pairs
        .into_iter()
        .filter_map(|pair| {
          self
            .open_positions
            .get(&determine_position_id(core_id, &pair))
            .map(Position::clone)
        })
        .collect(),
    )
  }

  pub fn get_all_open_positions(
    &mut self,
    core_id: Uuid,
  ) -> Result<Vec<Position>, DatabaseError> {
    Ok(
      self
        .open_positions
        .iter()
        .filter(|(position_id, _)| position_id.contains(&core_id.to_string()))
        .map(|(_, position)| Position::clone(position))
        .collect(),
    )
  }

  pub fn remove_position(
    &mut self,
    position_id: &String,
  ) -> Result<Option<Position>, DatabaseError> {
    Ok(self.open_positions.remove(position_id))
  }

  pub fn set_exited_position(
    &mut self,
    core_id: Uuid,
    position: Position,
  ) -> Result<(), DatabaseError> {
    let exited_positions_key = determine_exited_positions_id(core_id);
    match self.closed_positions.get_mut(&exited_positions_key) {
      None => {
        self.closed_positions.insert(exited_positions_key, vec![position]);
      },
      Some(closed_positions) => closed_positions.push(position),
    }
    Ok(())
  }

  pub fn get_exited_positions(
    &mut self,
    core_id: Uuid,
  ) -> Result<Vec<Position>, DatabaseError> {
    Ok(
      self
        .closed_positions
        .get(&determine_exited_positions_id(core_id))
        .map(Vec::clone)
        .unwrap_or_else(Vec::new),
    )
  }

  pub async fn add_candles(
    &mut self,
    pair: Pair,
    candles: Vec<Candle>,
  ) -> Result<(), DatabaseError> {
    let connection = DB_POOL.get().unwrap();
    let mut tx = connection.begin().await?;
    for candle in candles {
      sqlx::query(
                r#"
                INSERT OR REPLACE INTO candles(asset, open_time, open, high, low, close, close_time, volume, trade_count)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(pair.to_string())
            .bind(candle.open_time)
            .bind(candle.open)
            .bind(candle.high)
            .bind(candle.low)
            .bind(candle.close)
            .bind(candle.close_time)
            .bind(candle.volume)
            .bind(candle.trade_count)
            .execute(tx.as_mut())
            .await?;
    }
    tx.commit().await?;
    Ok(())
  }

  pub async fn fetch_all_candles(
    &mut self,
    pair: Pair,
  ) -> Result<Vec<Candle>, DatabaseError> {
    let connection = DB_POOL.get().unwrap();
    let candles: Vec<Candle> = sqlx::query_as("SELECT * FROM candles WHERE asset = ?1")
      .bind(pair.to_string())
      .fetch_all(connection)
      .await?;
    Ok(candles)
  }

  pub fn set_statistics(
    &mut self,
    core_id: Uuid,
    statistic: TradingSummary,
  ) -> Result<(), DatabaseError> {
    self.statistics.insert(core_id, statistic);
    Ok(())
  }

  pub fn generate_run_overview(
    &mut self,
    core_id: &Uuid,
    pair: &Pair,
  ) -> Result<Vec<LabelValueItem<String>>, DatabaseError> {
    let duration = if let Some(stats) = self.statistics.get(core_id) {
      Utc::now() - stats.starting_time
    } else {
      Duration::nanoseconds(0)
    };
    let open_trades = self.get_open_positions(core_id, vec![pair.clone().to_owned()]);
    let closed_positions = self.get_exited_positions(core_id.clone().to_owned());
    let n_closed_positions = {
      if let Ok(trades) = closed_positions {
        trades.len()
      } else {
        0
      }
    };

    let balance = if let Ok(balance) = self.get_balance(core_id.clone().to_owned()) {
      balance.total.to_string()
    } else {
      "No balance available.".to_string()
    };
    let rows: Vec<LabelValueItem<String>> = vec![
      LabelValueItem::new("Pair".to_string(), pair.to_string()),
      LabelValueItem::new(
        "Duration".to_string(),
        format!("{}", duration_to_readable(&duration)),
      ),
      LabelValueItem::new("Balance".to_string(), balance),
      LabelValueItem::new("Trades".to_string(), (n_closed_positions).to_string()),
    ];
    Ok(rows)
  }

  pub fn get_statistics(
    &mut self,
    core_id: &Uuid,
  ) -> Result<TradingSummary, DatabaseError> {
    let keys = self.statistics.keys();
    self.statistics.get(core_id).copied().ok_or(DatabaseError::DataMissing(format!(
      "Statistics for {} missing on database lookup. Available keys: {:?}",
      core_id, keys
    )))
  }

  pub async fn run(
    &mut self,
    pairs: Vec<Pair>,
    binance_client: BinanceClient,
  ) -> Result<(), DatabaseError> {
    log::info!("Database loop started.");
    let stream_url = self.stream_url.clone();
    let mut ticker = asset_ticker::new_ticker(pairs, &self.stream_url).await?;
    let mut account_listener =
      new_account_stream(&self.stream_url, binance_client).await?;
    loop {
      match ticker.try_recv() {
        Ok(event) => {
          self.event_tx.send(Event::Market(event.clone()));
          match event.detail {
            MarketEventDetail::Candle(candle) => {
              let candles: Vec<Candle> = vec![candle];
              let insert = self.add_candles(event.pair, candles).await;
              match insert {
                Ok(_) => log::info!("Inserted new candle."),
                Err(e) => log::warn!("Error inserting candle: {:?}", e),
              }
            },
            _ => (),
          }
        },
        Err(e) => {
          log::error!("error: {}", e);
          break;
        },
      }
      match account_listener.try_recv() {
        Ok(balances) => self.set_exchange_balances(balances),
        Err(e) => {
          log::error!("Error: {}", e);
          break;
        },
      }
    }
    Ok(())
  }
}

pub type ExitedPositionsId = String;
pub fn determine_exited_positions_id(core_id: Uuid) -> ExitedPositionsId {
  format!("positions_exited_{}", core_id)
}
