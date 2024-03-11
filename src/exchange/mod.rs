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
use binance_spot_connector_rust::http::request::RequestBuilder;
use binance_spot_connector_rust::http::Method;
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
                    ?;
                klines = data
                    .into_body_str()
                    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))
                    ?;
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

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, PartialOrd)]
pub struct ExchangeAllCoinsInfo {
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
  created_at: DateTime<Utc>,
}

impl ExchangeBalance {
  pub fn new(balances: Vec<ExchangeAssetBalance>) -> Self {
    let btc_valuation =
      balances.iter().fold(0f64, |acc, balance| acc + balance.btc_valuation);
    Self { btc_valuation, balances, created_at: Utc::now() }
  }
  pub fn btc_valuation(&self) -> f64 {
    self.btc_valuation
  }
}

async fn get_exchange_balances(
  binance_client: BinanceClient,
  use_testnet: bool,
) -> Result<Vec<ExchangeAssetBalance>, ExchangeError> {
  let balances = if use_testnet {
    get_exchange_balances_testnet(binance_client).await?
  } else {
    get_exchange_balances_realnet(binance_client).await?
  };
  Ok(balances)
}

async fn get_exchange_balances_testnet(
  binance_client: BinanceClient,
) -> Result<Vec<ExchangeAssetBalance>, ExchangeError> {
  let timestamp = Utc::now().timestamp().to_string();
  let params: Vec<(&str, &str)> = vec![("timestamp", &timestamp)];
  let request =
    RequestBuilder::new(Method::Post, "/sapi/v1/capital/config/getall").params(params);
  let data = binance_client
    .client
    .send(request)
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let balances = data
    .into_body_str()
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;

  let balances: Vec<ExchangeAssetBalance> = serde_json::from_str(&balances)?;
  Ok(balances)
}

async fn get_exchange_balances_realnet(
  binance_client: BinanceClient,
) -> Result<Vec<ExchangeAssetBalance>, ExchangeError> {
  let request =
    RequestBuilder::new(Method::Post, "/sapi/v3/asset/getUserAsset").params(vec![
      ("needBtcValuation", "true"),
      ("timestamp", &Utc::now().timestamp().to_string()),
    ]);
  let data = binance_client
    .client
    .send(request)
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let balances = data
    .into_body_str()
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;

  let balances: Vec<ExchangeAssetBalance> = serde_json::from_str(&balances)?;
  Ok(balances)
}

pub struct ExchangeBook {
  last_balance: Option<ExchangeBalance>,
  db: Arc<Mutex<Database>>,
  exchange_client: BinanceClient,
  use_testnet: bool,
}

impl ExchangeBook {
  pub fn new(
    db: Arc<Mutex<Database>>,
    exchange_client: BinanceClient,
    use_testnet: bool,
  ) -> Self {
    Self { db, exchange_client, last_balance: None, use_testnet }
  }
  pub async fn sync(&mut self) -> Result<(), ExchangeError> {
    let sync = if let Some(last_balance) = self.last_balance.clone() {
      last_balance.created_at + Duration::seconds(5) < Utc::now()
    } else {
      self.last_balance.is_none()
    };
    if sync {
      let balances =
        get_exchange_balances(self.exchange_client.clone(), self.use_testnet).await?;
      let mut db = self.db.lock().await;
      let exchange_balance = ExchangeBalance::new(balances);
      db.set_exchange_balance(exchange_balance.clone());
      self.last_balance = Some(exchange_balance);
      Ok(())
    } else {
      Ok(())
    }
  }

  pub fn last_balance_info(&self) -> (f64, DateTime<Utc>) {
    if let Some(exchange_balance) = self.last_balance.clone() {
      (exchange_balance.btc_valuation, exchange_balance.created_at)
    } else {
      (0.0, Utc::now())
    }
  }
}
