use serde::Deserialize;
use sqlx::FromRow;

#[derive(Deserialize, Debug, Clone, FromRow)]
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
