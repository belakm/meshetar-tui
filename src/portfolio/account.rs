use super::balance::Balance;
use crate::{
  assets::Pair,
  events::Event,
  exchange::{binance_client::BinanceClient, error::ExchangeError, ExchangeAccount},
};
use binance_spot_connector_rust::tokio_tungstenite::BinanceWebSocketClient;
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[derive(Deserialize, Debug, Clone)]
pub struct Account {
  #[serde(rename = "makerCommission")]
  pub maker_commission: i64,
  #[serde(rename = "takerCommission")]
  pub taker_commission: i64,
  #[serde(rename = "buyerCommission")]
  pub buyer_commission: i64,
  #[serde(rename = "sellerCommission")]
  pub seller_commission: i64,
  #[serde(rename = "canTrade")]
  pub can_trade: bool,
  #[serde(rename = "canWithdraw")]
  pub can_withdraw: bool,
  #[serde(rename = "canDeposit")]
  pub can_deposit: bool,
  pub brokered: bool,
  #[serde(rename = "requireSelfTradePrevention")]
  pub require_self_rade_prevention: bool,
  #[serde(rename = "preventSor")]
  pub prevent_sor: bool,
  #[serde(rename = "updateTime")]
  pub update_time: i64,
  #[serde(rename = "accountType")]
  pub account_type: String,
  // permissions: Vec<String>,
  pub uid: i64,
}
#[derive(Deserialize, Debug, Clone)]
struct ExchangeAccountBalance {
  a: String, //Asset
  f: f64,    //Free
  l: f64,    //Locked
}
impl ExchangeAccountBalance {
  pub fn to_balance(&self) -> (String, Balance) {
    let balance = Balance { time: Utc::now(), total: self.f + self.l, available: self.f };
    (self.a.clone(), balance)
  }
}
#[derive(Deserialize, Debug, Clone)]
struct ExchangeAccountUpdate {
  e: String, //Event type
  E: u64,    //Event Time
  u: u64,    //Time of last account update
  B: Vec<ExchangeAccountBalance>,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct AccountEvent {
  pub time: DateTime<Utc>,
  pub data: ExchangeAccount,
}

pub async fn new_account_stream(
  stream_url: &str,
  binance_client: BinanceClient,
) -> Result<UnboundedReceiver<Vec<(String, Balance)>>, ExchangeError> {
  let (tx, rx) = mpsc::unbounded_channel();
  let (mut conn, _) = BinanceWebSocketClient::connect_async(stream_url)
    .map_err(|e| ExchangeError::BinanceStreamError(e.to_string()))
    .await?;
  let key = binance_client
    .client
    .send(binance_spot_connector_rust::stream::new_listen_key())
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let key = binance_client.get_stream_key().await?;
  let stream = binance_spot_connector_rust::user_data_stream::user_data(&key);
  conn.subscribe(vec![&stream.into()]).await;

  tokio::spawn(async move {
    while let Some(message) = conn.as_mut().next().await {
      match message {
        Ok(message) => {
          let data = message.into_data();
          let string_data = String::from_utf8(data).expect("Found invalid UTF-8 chars");
          let raw_event_parse: Result<ExchangeAccountUpdate, serde_json::Error> =
            serde_json::from_str(&string_data);
          match raw_event_parse {
            Ok(ev) => {
              let balances: Vec<(String, Balance)> =
                ev.B.iter().map(|b| b.to_balance()).collect();
              if let Err(e) = tx.send(balances) {
                log::error!("Stopping spot account websocket: {:?}", e);
                break;
              }
            },
            Err(e) => {
              log::warn!("Error parsing asset feed event: {}", e);
            },
          }
        },
        Err(e) => log::warn!("Error recieving on PRICE SOCKET: {:?}", e),
      }
    }
  });

  Ok(rx)
}
