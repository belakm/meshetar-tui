use crate::{
  assets::Pair,
  events::Event,
  exchange::{binance_client::BinanceClient, error::ExchangeError},
  portfolio::balance::Balance,
  utils::serde_utils::f64_from_string,
};
use binance_spot_connector_rust::{
  http::request::RequestBuilder, tokio_tungstenite::BinanceWebSocketClient,
};
use chrono::{DateTime, Utc};
use futures::{StreamExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[derive(Deserialize, Debug, Clone)]
struct ExchangeAccountBalance {
  a: String,
  #[serde(deserialize_with = "f64_from_string")]
  f: f64,
  #[serde(deserialize_with = "f64_from_string")]
  l: f64,
}
impl ExchangeAccountBalance {
  pub fn to_balance(&self) -> (String, Balance) {
    let balance = Balance { time: Utc::now(), total: self.f + self.l, available: self.f };
    (self.a.clone(), balance)
  }
}
#[derive(Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
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
      log::info!("MESSAGE {:?}", message);
      match message {
        Ok(message) => {
          let data = message.into_data();
          if let Ok(string_data) = String::from_utf8(data) {
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
                log::warn!("Error parsing event on spot account feed: {}", e);
              },
            }
          }
        },
        Err(e) => log::warn!("Error recieving on spot account socket: {:?}", e),
      }
    }
  });
  Ok(rx)
}

#[derive(Debug, Serialize, Deserialize)]
struct ExchangeAccountBalanceFromRest {
  asset: String,
  #[serde(deserialize_with = "f64_from_string")]
  free: f64,
  #[serde(deserialize_with = "f64_from_string")]
  locked: f64,
}

impl ExchangeAccountBalanceFromRest {
  pub fn to_balance(&self) -> (String, Balance) {
    let balance =
      Balance { time: Utc::now(), total: self.free + self.locked, available: self.free };
    (self.asset.clone(), balance)
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawExchangeAccount {
  maker_commission: i64,
  taker_commission: i64,
  buyer_commission: i64,
  seller_commission: i64,
  balances: Vec<ExchangeAccountBalanceFromRest>,
  can_trade: bool,
  can_withdraw: bool,
  can_deposit: bool,
  brokered: bool,
  require_self_trade_prevention: bool,
  prevent_sor: bool,
  update_time: i64,
  account_type: String,
  permissions: Vec<String>,
  uid: i64,
}

impl RawExchangeAccount {
  pub fn to_exchange_account(&self) -> ExchangeAccount {
    let balances: Vec<(String, Balance)> =
      self.balances.iter().map(|b| b.to_balance()).collect();
    ExchangeAccount {
      maker_commission: self.maker_commission as f64 / 10000.0,
      taker_commission: self.taker_commission as f64 / 10000.0,
      buyer_commission: self.buyer_commission as f64 / 10000.0,
      seller_commission: self.seller_commission as f64 / 10000.0,
      balances,
      can_trade: self.can_trade,
      can_withdraw: self.can_withdraw,
      can_deposit: self.can_deposit,
      brokered: self.brokered,
      require_self_trade_prevention: self.require_self_trade_prevention,
      prevent_sor: self.prevent_sor,
      update_time: self.update_time,
      account_type: self.account_type.clone(),
      permissions: self.permissions.clone(),
      uid: self.uid,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Default)]
pub struct ExchangeAccount {
  maker_commission: f64,
  taker_commission: f64,
  buyer_commission: f64,
  seller_commission: f64,
  balances: Vec<(String, Balance)>,
  can_trade: bool,
  can_withdraw: bool,
  can_deposit: bool,
  brokered: bool,
  require_self_trade_prevention: bool,
  prevent_sor: bool,
  update_time: i64,
  account_type: String,
  permissions: Vec<String>,
  uid: i64,
}
impl ExchangeAccount {
  pub fn get_balances(&self) -> Vec<(String, Balance)> {
    self.balances.clone()
  }
}

pub async fn get_account_from_exchange(
  binance_client: BinanceClient,
) -> Result<ExchangeAccount, ExchangeError> {
  let request = binance_spot_connector_rust::trade::account().recv_window(5000);
  let res = binance_client
    .client
    .send(request)
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let res = res
    .into_body_str()
    .map_err(|e| ExchangeError::BinanceClientError(format!("{:?}", e)))?;
  let account: RawExchangeAccount =
    serde_json::from_str(&res).map_err(|e| ExchangeError::JsonSerDe(e))?;
  Ok(account.to_exchange_account())
}
