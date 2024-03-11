pub mod asset_ticker;
pub mod backtest_ticker;
// pub mod book;
pub mod error;
// pub mod routes;

use self::{asset_ticker::KlineEvent, error::AssetError};
use crate::{
  database::Database,
  exchange::{error::ExchangeError, BinanceKline},
  strategy::Signal,
  utils::{
    binance_client::BinanceClient,
    formatting::{dt_to_readable, timestamp_to_dt},
  },
};
use binance_spot_connector_rust::market::klines::KlineInterval;
use chrono::{DateTime, Duration, Utc};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::{sync::Arc, thread::sleep};
use strum::{Display, EnumString};
use tokio::sync::{mpsc, Mutex};
use tracing::info;

#[derive(
  PartialEq,
  Default,
  Display,
  Debug,
  Hash,
  Eq,
  Clone,
  Copy,
  Serialize,
  Deserialize,
  PartialOrd,
  EnumString,
)]
pub enum Pair {
  #[default]
  BTCUSDT,
  ETHBTC,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Feed {
  Next(MarketEvent),
  Empty,
  Unhealthy,
  Finished,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct MarketEvent {
  pub time: DateTime<Utc>,
  pub asset: Pair,
  pub detail: MarketEventDetail,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub enum MarketEventDetail {
  Trade(PublicTrade),
  OrderBookL1(OrderBookL1),
  Candle(Candle),
  BacktestCandle((Candle, Option<Signal>)),
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Liquidation {
  pub side: Side,
  pub price: f64,
  pub quantity: f64,
  pub time: DateTime<Utc>,
}

#[derive(FromRow, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Candle {
  pub open_time: DateTime<Utc>,
  pub close_time: DateTime<Utc>,
  pub open: f64,
  pub high: f64,
  pub low: f64,
  pub close: f64,
  pub volume: f64,
  pub trade_count: i64,
}

impl From<&KlineEvent> for Candle {
  fn from(kline: &KlineEvent) -> Self {
    Candle {
      open_time: timestamp_to_dt(kline.detail.open_time),
      close_time: timestamp_to_dt(kline.detail.close_time),
      open: kline.detail.open_price,
      high: kline.detail.high_price,
      low: kline.detail.low_price,
      close: kline.detail.close_price,
      volume: kline.detail.base_volume,
      trade_count: kline.detail.trade_count,
    }
  }
}

impl From<&BinanceKline> for Candle {
  fn from(kline: &BinanceKline) -> Self {
    Candle {
      open_time: timestamp_to_dt(kline.0),
      close_time: timestamp_to_dt(kline.6),
      open: kline.1.parse().unwrap(),
      high: kline.2.parse().unwrap(),
      low: kline.3.parse().unwrap(),
      close: kline.4.parse().unwrap(),
      volume: kline.5.parse().unwrap(),
      trade_count: kline.8,
    }
  }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct Level {
  pub price: f64,
  pub amount: f64,
}
#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct OrderBookL1 {
  pub last_update_time: DateTime<Utc>,
  pub best_bid: Level,
  pub best_ask: Level,
}
impl OrderBookL1 {
  pub fn mid_price(&self) -> f64 {
    (self.best_bid.price + self.best_ask.price) / 2.0
  }
  pub fn volume_weighted_mid_price(&self) -> f64 {
    (self.best_bid.price * self.best_bid.amount
      + self.best_ask.price * self.best_ask.amount)
      / (self.best_bid.amount + self.best_ask.amount)
  }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct PublicTrade {
  pub id: String,
  pub price: f64,
  pub amount: f64,
  pub side: Side,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Side {
  Buy,
  Sell,
}

pub struct MarketFeed {
  pub market_receiver: Option<mpsc::UnboundedReceiver<MarketEvent>>,
  is_live: bool,
  database: Arc<Mutex<Database>>,
  last_n_candles: usize,
  pair: Pair,
  model_name: String,
  stream_url: String,
}
impl MarketFeed {
  pub fn next(&mut self) -> Feed {
    if self.market_receiver.is_none() {
      return Feed::Unhealthy;
    }
    match self.market_receiver.as_mut().unwrap().try_recv() {
      Ok(event) => Feed::Next(event),
      Err(mpsc::error::TryRecvError::Empty) => Feed::Empty,
      Err(mpsc::error::TryRecvError::Disconnected) => Feed::Finished,
    }
  }
  pub async fn run(&mut self) -> Result<(), AssetError> {
    self.market_receiver = if self.is_live {
      Some(self.new_live_feed(self.pair.clone()).await?)
    } else {
      Some(
        self
          .new_backtest(
            self.database.clone(),
            self.last_n_candles,
            50,
            self.pair.clone(),
            self.model_name.clone(),
          )
          .await?,
      )
    };
    info!(
      "Datafeed init complete. Market receiver is ok: {}",
      self.market_receiver.is_some()
    );
    Ok(())
  }
  async fn new_live_feed(
    &self,
    pair: Pair,
  ) -> Result<mpsc::UnboundedReceiver<MarketEvent>, ExchangeError> {
    let ticker = asset_ticker::new_ticker(self.pair.clone(), &self.stream_url).await?;
    Ok(ticker)
  }
  async fn new_backtest(
    &self,
    database: Arc<Mutex<Database>>,
    last_n_candles: usize,
    buffer_n_of_candles: usize,
    pair: Pair,
    model_name: String,
  ) -> Result<mpsc::UnboundedReceiver<MarketEvent>, AssetError> {
    let ticker = backtest_ticker::new_ticker(
      database,
      last_n_candles,
      buffer_n_of_candles,
      pair,
      model_name,
    )
    .await?;
    Ok(ticker)
  }
  pub fn new(
    is_live: bool,
    database: Arc<Mutex<Database>>,
    last_n_candles: usize,
    pair: Pair,
    model_name: String,
    stream_url: String,
  ) -> Self {
    MarketFeed {
      market_receiver: None,
      is_live,
      database,
      last_n_candles,
      pair,
      model_name,
      stream_url,
    }
  }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct MarketMeta {
  pub close: f64,
  pub time: DateTime<Utc>,
}

impl Default for MarketMeta {
  fn default() -> Self {
    Self { close: 100.0, time: Utc::now() }
  }
}
