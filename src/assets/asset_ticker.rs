use super::{error::AssetError, Candle, MarketEvent, MarketEventDetail, Pair};
use crate::{exchange::error::ExchangeError, utils::serde_utils::f64_from_string};
use binance_spot_connector_rust::{
  market::klines::KlineInterval, market_stream::kline::KlineStream,
  tokio_tungstenite::BinanceWebSocketClient,
};
use chrono::{TimeZone, Utc};
use futures::{StreamExt, TryFutureExt};
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::mpsc::{self, error::SendError, UnboundedReceiver};
use tracing::{info, warn};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct KlineEvent {
  pub e: String, // Event type
  pub E: i64,    // Event time
  #[serde(rename = "s")]
  pub symbol: String, // Symbol
  #[serde(rename = "k")]
  pub detail: KlineDetail,
}

#[derive(Debug, Deserialize)]
pub struct KlineDetail {
  #[serde(rename = "t")]
  pub open_time: i64, // Kline start time
  #[serde(rename = "T")]
  pub close_time: i64, // Kline close time
  #[serde(rename = "i")]
  pub interval: String, // Interval
  #[serde(rename = "f")]
  pub first_trade_id: i64, // First trade ID
  #[serde(rename = "L")]
  pub last_trade_id: i64, // Last trade ID
  #[serde(rename = "o", deserialize_with = "f64_from_string")]
  pub open_price: f64, // Open price
  #[serde(rename = "c", deserialize_with = "f64_from_string")]
  pub close_price: f64, // Close price
  #[serde(rename = "h", deserialize_with = "f64_from_string")]
  pub high_price: f64, // High price
  #[serde(rename = "l", deserialize_with = "f64_from_string")]
  pub low_price: f64, // Low price
  #[serde(rename = "v", deserialize_with = "f64_from_string")]
  pub base_volume: f64, // Base asset volume
  #[serde(rename = "n")]
  pub trade_count: i64, // Number of trades
  #[serde(rename = "x")]
  pub is_closed: bool, // Is this kline closed?
  #[serde(rename = "q")]
  pub quote_volume: String, // Quote asset volume
  #[serde(rename = "V", deserialize_with = "f64_from_string")]
  pub taker_buy_base: f64, // Taker buy base asset volume
  #[serde(rename = "Q", deserialize_with = "f64_from_string")]
  pub taker_buy_quote: f64, // Taker buy quote asset volume
  #[serde(rename = "B")]
  pub ignore: String, // Ignore
}

pub async fn new_ticker(
  pairs: Vec<Pair>,
  stream_url: &str,
) -> Result<UnboundedReceiver<MarketEvent>, ExchangeError> {
  let (tx, rx) = mpsc::unbounded_channel();
  let (mut conn, _) = BinanceWebSocketClient::connect_async(stream_url)
    .map_err(|e| ExchangeError::BinanceStreamError(e.to_string()))
    .await?;

  for pair in pairs {
    conn
      .subscribe(vec![
        &KlineStream::new(&pair.to_string(), KlineInterval::Minutes1).into()
      ])
      .await;
  }

  tokio::spawn(async move {
    while let Some(message) = conn.as_mut().next().await {
      match message {
        Ok(message) => {
          let data = message.into_data();
          if let Ok(string_data) = String::from_utf8(data) {
            let raw_asset_parse: Result<KlineEvent, serde_json::Error> =
              serde_json::from_str(&string_data);
            match raw_asset_parse {
              Ok(new_kline) => {
                if let Ok(pair) = Pair::from_str(&new_kline.symbol) {
                  if let Err(e) = tx.send(MarketEvent {
                    time: Utc.timestamp_opt(new_kline.E, 0).unwrap(),
                    pair,
                    detail: MarketEventDetail::Candle(Candle::from(&new_kline)),
                  }) {
                    let e_msg = e.to_string();
                    match e {
                      SendError(market_event) => {
                        log::error!("Mystery market feed error: {}", e_msg);
                        break;
                      },
                    }
                  };
                } else {
                  log::warn!("Couldn't parse Pair from websocket kline.")
                };
              },
              Err(e) => {
                warn!("Error parsing asset feed event: {}", e);
              },
            }
          }
        },
        Err(e) => warn!("Error recieving on PRICE SOCKET: {:?}", e),
      }
    }
  });

  Ok(rx)
}
