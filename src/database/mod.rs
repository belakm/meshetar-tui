pub mod error;
pub mod sqlite;

use self::{error::DatabaseError, sqlite::DB_POOL};
use crate::{
    assets::{Asset, Candle},
    portfolio::{
        account::Account,
        balance::{
            Balance, BalanceId, ExchangeBalance, ExchangeBalanceAsset, ExchangeBalanceSheet,
        },
        position::{determine_position_id, Position, PositionId},
    },
    statistic::TradingSummary,
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

pub struct Database {
    open_positions: HashMap<PositionId, Position>,
    closed_positions: HashMap<String, Vec<Position>>,
    current_balances: HashMap<BalanceId, Balance>,
    statistics: HashMap<Asset, TradingSummary>,
}
impl Database {
    pub async fn new() -> Result<Database, DatabaseError> {
        sqlite::initialize().await?;
        Ok(Database {
            open_positions: HashMap::new(),
            closed_positions: HashMap::new(),
            current_balances: HashMap::new(),
            statistics: HashMap::new(),
        })
    }

    pub async fn set_exchange_balance(
        &mut self,
        balance: ExchangeBalance,
    ) -> Result<(), DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        let mut tx = connection.begin().await?;
        let timestamp: String = DateTime::to_rfc3339(&Utc::now());
        let balance_sheet: ExchangeBalanceSheet = sqlx::query_as(
        "INSERT INTO balance_sheets (timestamp, btc_valuation, busd_valuation) VALUES (?1, 0.0, 0.0) RETURNING *",
    )
    .bind(timestamp)
    .fetch_one(connection)
    .await.map_err(|_| DatabaseError::ReadError)?;

        // Insert snapshot data
        for balance in balance.balances {
            sqlx::query(
                "INSERT INTO balances (asset, free, locked, balance_sheet_id, btc_valuation) 
                VALUES (
                    ?1, 
                    ?2, 
                    ?3, 
                    ?4,
                        CASE WHEN ?1 = 'BTC' THEN ?2 -- get valuation from ticker otherwise default 
                        ELSE COALESCE((SELECT last_price FROM asset_ticker WHERE symbol = ?5), 0) * ?6
                        END
                    )",
            )
            .bind(&balance.asset)
            .bind(&balance.free)
            .bind(&balance.locked)
            .bind(&balance_sheet.id)
            .bind(&format!("{}{}", balance.asset.to_string(), "BTC"))
            .bind(&balance.free)
            .execute(tx.as_mut())
            .await?;
        }

        sqlx::query(
            "UPDATE balance_sheets
            SET btc_valuation = (
                SELECT SUM(btc_valuation)
                FROM balances
                WHERE balance_sheet_id = ?1
            ),
            busd_valuation = (
                SELECT SUM(btc_valuation) * asset_ticker.last_price
                FROM balances
                LEFT JOIN asset_ticker ON asset_ticker.symbol = 'BTCBUSD'
                WHERE balance_sheet_id = ?1
            )
            WHERE id = ?1",
        )
        .bind(&balance_sheet.id)
        .execute(tx.as_mut())
        .await?;

        tx.commit().await?;

        // Commit transaction
        Ok(())
    }

    pub async fn get_exchange_balance(&mut self) -> Result<ExchangeBalance, DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        let balance_sheet: ExchangeBalanceSheet = sqlx::query_as(
            "SELECT * FROM balance_sheets WHERE id = (SELECT MAX(id) FROM balance_sheets)",
        )
        .fetch_one(connection)
        .await
        .map_err(|_| DatabaseError::ReadError)?;

        let query = &format!(
            "SELECT * 
            FROM balances
            WHERE balance_sheet_id = {:?}",
            &balance_sheet.id
        );
        let balances: Vec<ExchangeBalanceAsset> = sqlx::query_as(query)
            .fetch_all(connection)
            .await
            .map_err(|_| DatabaseError::ReadError)?;

        Ok(ExchangeBalance {
            timestamp: balance_sheet.timestamp,
            btc_valuation: balance_sheet.btc_valuation,
            busd_valuation: balance_sheet.busd_valuation,
            balances,
        })
    }

    pub async fn set_account(&mut self, account: Account) -> Result<(), DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        sqlx::query(
            "INSERT OR REPLACE INTO account (
            maker_commission,
            taker_commission,
            buyer_commission,
            seller_commission,
            can_trade,
            can_withdraw,
            can_deposit,
            brokered,
            require_self_rade_prevention,
            prevent_sor,
            update_time,
            account_type,
            uid
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .bind(account.maker_commission)
        .bind(account.taker_commission)
        .bind(account.buyer_commission)
        .bind(account.seller_commission)
        .bind(account.can_trade)
        .bind(account.can_withdraw)
        .bind(account.can_deposit)
        .bind(account.brokered)
        .bind(account.require_self_rade_prevention)
        .bind(account.prevent_sor)
        .bind(account.update_time)
        .bind(account.account_type.clone())
        .bind(account.uid)
        .execute(connection)
        .await?;
        Ok(())
    }

    pub async fn get_account(&mut self, uid: i64) -> Result<Account, DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        let account: Account = sqlx::query_as("SELECT * FROM account WHERE uid = $1")
            .bind(uid)
            .fetch_one(connection)
            .await
            .map_err(|_| DatabaseError::ReadError)?;
        Ok(account)
    }

    pub fn set_balance(&mut self, core_id: Uuid, balance: Balance) -> Result<(), DatabaseError> {
        self.current_balances
            .insert(Balance::balance_id(core_id), balance);
        Ok(())
    }

    pub fn get_balance(&mut self, core_id: Uuid) -> Result<Balance, DatabaseError> {
        self.current_balances
            .get(&Balance::balance_id(core_id))
            .copied()
            .ok_or(DatabaseError::DataMissing)
    }

    pub fn set_open_position(&mut self, position: Position) -> Result<(), DatabaseError> {
        self.open_positions
            .insert(position.position_id.clone(), position);
        Ok(())
    }

    pub fn get_open_position(
        &mut self,
        position_id: &PositionId,
    ) -> Result<Option<Position>, DatabaseError> {
        Ok(self.open_positions.get(position_id).map(Position::clone))
    }

    pub fn get_open_positions(
        &mut self,
        core_id: Uuid,
        assets: Vec<Asset>,
    ) -> Result<Vec<Position>, DatabaseError> {
        Ok(assets
            .into_iter()
            .filter_map(|asset| {
                self.open_positions
                    .get(&determine_position_id(core_id, &asset))
                    .map(Position::clone)
            })
            .collect())
    }

    pub fn get_all_open_positions(
        &mut self,
        core_id: Uuid,
    ) -> Result<Vec<Position>, DatabaseError> {
        Ok(self
            .open_positions
            .iter()
            .filter(|(position_id, _)| position_id.contains(&core_id.to_string()))
            .map(|(_, position)| Position::clone(position))
            .collect())
    }

    pub fn remove_position(
        &mut self,
        position_id: &String,
    ) -> Result<Option<Position>, DatabaseError> {
        Ok(self.open_positions.remove(position_id))
    }

    pub fn set_exited_position(
        &mut self,
        core_id: Uuid,
        position: Position,
    ) -> Result<(), DatabaseError> {
        let exited_positions_key = determine_exited_positions_id(core_id);

        match self.closed_positions.get_mut(&exited_positions_key) {
            None => {
                self.closed_positions
                    .insert(exited_positions_key, vec![position]);
            }
            Some(closed_positions) => closed_positions.push(position),
        }
        Ok(())
    }

    pub fn get_exited_positions(&mut self, core_id: Uuid) -> Result<Vec<Position>, DatabaseError> {
        Ok(self
            .closed_positions
            .get(&determine_exited_positions_id(core_id))
            .map(Vec::clone)
            .unwrap_or_else(Vec::new))
    }

    pub async fn add_candles(
        &mut self,
        asset: Asset,
        candles: Vec<Candle>,
    ) -> Result<(), DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        let mut tx = connection.begin().await?;
        for candle in candles {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO candles(asset, open_time, open, high, low, close, close_time, volume, trade_count)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(asset.to_string())
            .bind(candle.open_time)
            .bind(candle.open)
            .bind(candle.high)
            .bind(candle.low)
            .bind(candle.close)
            .bind(candle.close_time)
            .bind(candle.volume)
            .bind(candle.trade_count)
            .execute(tx.as_mut())
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn fetch_all_candles(&mut self, asset: Asset) -> Result<Vec<Candle>, DatabaseError> {
        let connection = DB_POOL.get().unwrap();
        let candles: Vec<Candle> = sqlx::query_as("SELECT * FROM candles WHERE asset = ?1")
            .bind(asset.to_string())
            .fetch_all(connection)
            .await?;
        Ok(candles)
    }

    pub fn set_statistics(
        &mut self,
        asset: Asset,
        statistic: TradingSummary,
    ) -> Result<(), DatabaseError> {
        self.statistics.insert(asset, statistic);
        Ok(())
    }

    pub fn get_statistics(&mut self, asset: &Asset) -> Result<TradingSummary, DatabaseError> {
        self.statistics
            .get(asset)
            .copied()
            .ok_or(DatabaseError::DataMissing)
    }
}

pub type ExitedPositionsId = String;
pub fn determine_exited_positions_id(core_id: Uuid) -> ExitedPositionsId {
    format!("positions_exited_{}", core_id)
}
