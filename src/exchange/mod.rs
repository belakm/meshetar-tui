pub mod account;
pub mod binance_client;
pub mod error;

use self::account::ExchangeAccount;
use self::binance_client::BinanceClient;
use self::error::ExchangeError;
use crate::assets::{MarketEvent, MarketEventDetail};
use crate::portfolio::balance::Balance;
use crate::utils::serde_utils::f64_default;
use crate::{
  assets::{error::AssetError, Candle, Pair},
  database::Database,
  utils::formatting::timestamp_to_dt,
};
use binance_spot_connector_rust::http::request::RequestBuilder;
use binance_spot_connector_rust::http::Method;
use binance_spot_connector_rust::{
  market::klines::KlineInterval, wallet::user_asset::UserAsset,
};
use chrono::{DateTime, Duration, Utc};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum ExchangeEvent {
  ExchangeAccount(ExchangeAccount),
  ExchangeBalanceUpdate(Vec<(String, Balance)>),
  Market(MarketEvent),
}

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
