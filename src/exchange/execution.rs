use super::{
  binance_client::{self, BinanceClient},
  error::ExchangeError,
};
use crate::{
  assets::{Pair, Side},
  utils::serde_utils::f64_from_string,
};
use binance_spot_connector_rust::trade::order::TimeInForce;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::FromPrimitive;
use serde::Deserialize;

pub struct ExchangeFill {
  pub qty: f64,
  pub updated_at: DateTime<Utc>,
  pub price: f64,
}

#[derive(Deserialize)]
pub struct ExchangeFillResponseFill {
  #[serde(deserialize_with = "f64_from_string")]
  price: f64,
  #[serde(deserialize_with = "f64_from_string")]
  qty: f64,
  #[serde(deserialize_with = "f64_from_string")]
  commission: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeFillResponse {
  //   "symbol": "BTCUSDT",
  // "orderId": 28,
  // "orderListId": -1, //Unless OCO, value will be -1
  // "clientOrderId": "6gCrw2kRUAF9CvJDGP16IP",
  transact_time: u64,
  // price: f64,
  // "origQty": "10.00000000",
  #[serde(deserialize_with = "f64_from_string")]
  executed_qty: f64,
  // "cummulativeQuoteQty": "10.00000000",
  status: String,
  fills: Vec<ExchangeFillResponseFill>,
  // "timeInForce": "GTC",
  // "type": "MARKET",
  // "side": "SELL",
  // "workingTime": 1507725176595,
  // "selfTradePreventionMode": "NONE"
}

pub fn fill_order(
  binance_client: &BinanceClient,
  pair: Pair,
  qty: f64,
  side: Side,
) -> Result<ExchangeFill, ExchangeError> {
  let truncated_qty = (qty * 100_000.0).round() / 100_000.0;
  let dec_qty = rust_decimal::Decimal::from_f64(truncated_qty).unwrap();
  let request = binance_spot_connector_rust::trade::new_order(
    &pair.to_string(),
    side.to_binance_side(),
    "MARKET",
  )
  .quantity(dec_qty);

  log::info!(
    "------ INTO REQ -------- dec: {}, qty: {:?}, side: {:?}",
    dec_qty,
    qty,
    side
  );

  let res = binance_client.client.send(request).map_err(|e| {
    ExchangeError::BinanceClientError(format!("Error on order fill: {:?}", e))
  })?;

  let res = res.into_body_str().map_err(|e| {
    ExchangeError::BinanceClientError(format!("Error parsing fill event res: {:?}", e))
  })?;

  log::info!("RES string: {:?}", res);

  let res: ExchangeFillResponse =
    serde_json::from_str(&res).map_err(|e| ExchangeError::JsonSerDe(e))?;
  let price = weighted_average_price(res.fills);
  if res.status == "FILLED" && price.is_some() {
    Ok(ExchangeFill {
      qty: res.executed_qty,
      updated_at: DateTime::from_timestamp_millis(res.transact_time as i64).unwrap(),
      price: price.unwrap(),
    })
  } else {
    Err(ExchangeError::UnfilledOrder)
  }
}

fn weighted_average_price(fills: Vec<ExchangeFillResponseFill>) -> Option<f64> {
  let total_weight: f64 = fills.iter().map(|fill| fill.qty).sum();
  if total_weight == 0.0 {
    return None;
  }
  let weighted_sum: f64 = fills.iter().map(|fill| fill.price * fill.qty).sum();
  Some(weighted_sum / total_weight)
}
