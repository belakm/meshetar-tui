pub mod asset_ticker;
pub mod backtest_ticker;
// pub mod book;
pub mod error;
// pub mod routes;

use self::{asset_ticker::KlineEvent, error::AssetError};
use crate::{
    database::Database,
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
use std::sync::Arc;
use strum::Display;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

#[derive(PartialEq, Display, Debug, Hash, Eq, Clone, Serialize, Deserialize, PartialOrd)]
pub enum Asset {
    BTCUSDT,
    ETHBTC,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub enum Feed {
    Next(MarketEvent),
    Unhealthy,
    Finished,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct MarketEvent {
    pub time: DateTime<Utc>,
    pub asset: Asset,
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
        (self.best_bid.price * self.best_bid.amount + self.best_ask.price * self.best_ask.amount)
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
    asset: Asset,
    database: Arc<Mutex<Database>>,
    last_n_candles: usize,
}
impl MarketFeed {
    pub fn next(&mut self) -> Feed {
        if self.market_receiver.is_none() {
            return Feed::Unhealthy;
        }
        loop {
            match self.market_receiver.as_mut().unwrap().try_recv() {
                Ok(event) => break Feed::Next(event),
                Err(mpsc::error::TryRecvError::Empty) => continue,
                Err(mpsc::error::TryRecvError::Disconnected) => break Feed::Finished,
            }
        }
    }
    pub async fn run(&mut self) -> Result<(), AssetError> {
        info!("Datafeed init.");
        self.market_receiver = if self.is_live {
            Some(self.new_live_feed(self.asset.clone()).await?)
        } else {
            Some(
                self.new_backtest(
                    self.asset.clone(),
                    self.database.clone(),
                    self.last_n_candles,
                    50,
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
        asset: Asset,
    ) -> Result<mpsc::UnboundedReceiver<MarketEvent>, AssetError> {
        let ticker = asset_ticker::new_ticker(asset).await?;
        Ok(ticker)
    }
    async fn new_backtest(
        &self,
        asset: Asset,
        database: Arc<Mutex<Database>>,
        last_n_candles: usize,
        buffer_n_of_candles: usize,
    ) -> Result<mpsc::UnboundedReceiver<MarketEvent>, AssetError> {
        let ticker =
            backtest_ticker::new_ticker(asset, database, last_n_candles, buffer_n_of_candles)
                .await?;
        Ok(ticker)
    }
    pub fn new(
        asset: Asset,
        is_live: bool,
        database: Arc<Mutex<Database>>,
        last_n_candles: usize,
    ) -> Self {
        MarketFeed {
            market_receiver: None,
            asset,
            is_live,
            database,
            last_n_candles,
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
        Self {
            close: 100.0,
            time: Utc::now(),
        }
    }
}

pub async fn fetch_candles(
    duration: Duration,
    asset: Asset,
    binance_client: Arc<BinanceClient>,
) -> Result<Vec<Candle>, AssetError> {
    let mut start_time: i64 = (Utc::now() - duration).timestamp_millis();
    let mut candles = Vec::<Candle>::new();
    loop {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                info!("Loading candles from: {:?}", timestamp_to_dt(start_time));
                let request = binance_spot_connector_rust::market::klines(&asset.to_string(), KlineInterval::Minutes1)
                    .start_time(start_time as u64)
                    .limit(1000);
                let klines;
                {
                    let data = binance_client.client
                        .send(request)
                        .map_err(|e| AssetError::BinanceClientError(format!("{:?}", e)))
                        .await?;
                    klines = data
                        .into_body_str()
                        .map_err(|e| AssetError::BinanceClientError(format!("{:?}", e)))
                        .await?;
                };

                let new_candles = parse_binance_klines(&klines).await?;
                let last_candle = &new_candles.last();
                if let Some(last_candle) = last_candle {
                    start_time = last_candle.close_time.timestamp_millis();
                    candles.extend(new_candles);// .concat(new_candles);
                } else {
                    break
                }
            }
        }
    }
    info!("Candles fetched: {}", candles.len());
    Ok(candles)
}

pub type BinanceKline = (
    i64,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    i64,
    String,
    String,
    String,
);

async fn parse_binance_klines(klines: &String) -> Result<Vec<Candle>, AssetError> {
    let data: Vec<BinanceKline> = serde_json::from_str(klines)?;
    let mut new_candles: Vec<Candle> = Vec::new();
    for candle in data {
        let new_candle = Candle::from(&candle);
        new_candles.push(Candle::from(new_candle));
    }
    Ok(new_candles)
}
