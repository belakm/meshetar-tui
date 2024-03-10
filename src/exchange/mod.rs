pub mod error;
use self::error::ExchangeError;
use crate::utils::serde_utils::f64_default;
use crate::{
  assets::{error::AssetError, Candle, Pair},
  database::Database,
  utils::{
    binance_client::{self, BinanceClient},
    formatting::timestamp_to_dt,
  },
};
use binance_spot_connector_rust::{
  market::klines::KlineInterval, wallet::user_asset::UserAsset,
};
use chrono::{DateTime, Duration, Utc};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn fetch_candles(
  duration: Duration,
  asset: Pair,
  binance_client: Arc<BinanceClient>,
) -> Result<Vec<Candle>, ExchangeError> {
  let mut start_time: i64 = (Utc::now() - duration).timestamp_millis();
  let mut candles = Vec::<Candle>::new();
  loop {
    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
            log::info!("Loading candles from: {:?}", timestamp_to_dt(start_time));
            let request = binance_spot_connector_rust::market::klines(&asset.to_string(), KlineInterval::Minutes1)
                .start_time(start_time as u64)
                .limit(1000);
            let klines;
            {
                let data = binance_client.client
                    .send(request)
                    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))
                    .await?;
                klines = data
                    .into_body_str()
                    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))
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
  log::info!("Candles fetched: {}", candles.len());
  Ok(candles)
}

pub type BinanceKline =
  (i64, String, String, String, String, String, i64, String, i64, String, String, String);

async fn parse_binance_klines(klines: &String) -> Result<Vec<Candle>, ExchangeError> {
  let data: Vec<BinanceKline> = serde_json::from_str(klines)?;
  let mut new_candles: Vec<Candle> = Vec::new();
  for candle in data {
    let new_candle = Candle::from(&candle);
    new_candles.push(Candle::from(new_candle));
  }
  Ok(new_candles)
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, PartialOrd)]
pub struct ExchangeAssetBalance {
  pub id: i64,
  pub asset: String,
  pub free: f64,
  pub locked: f64,
  pub balance_sheet_id: i64,
  #[serde(default = "f64_default")]
  pub btc_valuation: f64,
}

#[derive(Debug, Clone)]
pub struct ExchangeBalance {
  btc_valuation: f64,
  balances: Vec<ExchangeAssetBalance>,
}

impl ExchangeBalance {
  pub fn new(balances: Vec<ExchangeAssetBalance>) -> Self {
    let btc_valuation =
      balances.iter().fold(0f64, |acc, balance| acc + balance.btc_valuation);
    Self { btc_valuation, balances }
  }
}

async fn get_exchange_balances(
  binance_client: BinanceClient,
) -> Result<Vec<ExchangeAssetBalance>, ExchangeError> {
  let request =
    binance_spot_connector_rust::wallet::user_asset().need_btc_valuation(true);
  let data = binance_client
    .client
    .send(request)
    .await
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let balances = data
    .into_body_str()
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))
    .await?;

  let balances: Vec<ExchangeAssetBalance> = serde_json::from_str(&balances)?;
  Ok(balances)
}

pub struct ExchangeBook {
  last_balance_sync: DateTime<Utc>,
  db: Arc<Mutex<Database>>,
  exchange_client: BinanceClient,
}

impl ExchangeBook {
  pub fn new(db: Arc<Mutex<Database>>, exchange_client: BinanceClient) -> Self {
    Self { db, exchange_client, last_balance_sync: DateTime::<Utc>::default() }
  }
  pub async fn sync(&self) -> Result<(), ExchangeError> {
    if self.last_balance_sync + Duration::seconds(5) < Utc::now() {
      let balances = get_exchange_balances(self.exchange_client.clone()).await?;
      let mut db = self.db.lock().await;
      db.set_exchange_balance(ExchangeBalance::new(balances));
      Ok(())
    } else {
      Ok(())
    }
  }
}
