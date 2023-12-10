use sqlx::{Pool, Sqlite, SqlitePool};
use std::{fs::File, path::Path};
use tokio::sync::OnceCell;

use super::error::DatabaseError;

pub static DB_POOL: OnceCell<Pool<Sqlite>> = OnceCell::const_new();

pub async fn initialize() -> Result<(), DatabaseError> {
    println!("Initializing database.");
    match set_connection().await {
        Ok(_) => {
            setup_tables().await?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub async fn set_connection() -> Result<(), DatabaseError> {
    // Creates the database file if it doesnt exist
    let database_path = "database.sqlite";
    if Path::new(database_path).exists() == false {
        File::create(database_path).map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    }
    // Creates a new pool
    let pool = SqlitePool::connect("database.sqlite")
        .await
        .map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    DB_POOL
        .set(pool)
        .map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    Ok(())
}

pub async fn setup_tables() -> Result<(), DatabaseError> {
    let connection = DB_POOL.get();
    if let Some(connection) = connection {
        sqlx::query(
            "BEGIN;
        CREATE TABLE IF NOT EXISTS balances ( 
            core_id TEXT NOT NULL UNIQUE,
            asset TEXT NOT NULL UNIQUE,
            time DATETIME NOT NULL,
            total REAL NOT NULL,
            available REAL NOT NULL,
            PRIMARY KEY (core_id, asset)
        );
        CREATE TABLE IF NOT EXISTS exchange_balances (
            asset TEXT NOT NULL,
            free REAL NOT NULL,
            locked REAL NOT NULL,
            balance_sheet_id INTEGER,
            btc_valuation REAL NOT NULL,
            FOREIGN KEY (balance_sheet_id) REFERENCES balance_sheets (id)
            PRIMARY KEY (balance_sheet_id, asset)
        );
        CREATE TABLE IF NOT EXISTS balance_sheets (
            id INTEGER PRIMARY KEY,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            btc_valuation REAL NOT NULL,
            busd_valuation REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS candles (
            asset TEXT NOT NULL,
            open_time INTEGER NOT NULL, 
            open REAL NOT NULL, 
            high REAL NOT NULL,
            low REAL NOT NULL, 
            close REAL NOT NULL, 
            close_time INTEGER NOT NULL, 
            volume REAL NOT NULL, 
            trade_count INTEGER NOT NULL,
            PRIMARY KEY (open_time, asset)
        );
        CREATE TABLE IF NOT EXISTS indicators (
            symbol TEXT NOT NULL,
            interval TEXT NOT NULL,
            open_time INTEGER NOT NULL,
            adi REAL NOT NULL,
            cci REAL NOT NULL,
            dema REAL NOT NULL,
            dma REAL NOT NULL,
            ema REAL NOT NULL,
            hma REAL NOT NULL,
            rma REAL NOT NULL,
            sma REAL NOT NULL,
            smm REAL NOT NULL,
            swma REAL NOT NULL,
            tema REAL NOT NULL,
            tma REAL NOT NULL,
            tr REAL NOT NULL,
            trima REAL NOT NULL,
            tsi REAL NOT NULL,
            vwma REAL NOT NULL,
            vidya REAL NOT NULL,
            wma REAL NOT NULL,
            wsma REAL NOT NULL,
            PRIMARY KEY (open_time, symbol, interval)
        );
        CREATE TABLE IF NOT EXISTS signals (
            symbol TEXT NOT NULL,
            interval TEXT NOT NULL,
            time INTEGER NOT NULL,
            signal TEXT NOT NULL,
            PRIMARY KEY (symbol, interval, time)
        );
        CREATE TABLE IF NOT EXISTS account(
            maker_commission INTEGER NOT NULL,
            taker_commission INTEGER NOT NULL,
            buyer_commission INTEGER NOT NULL,
            seller_commission INTEGER NOT NULL,
            can_trade INTEGER NOT NULL,
            can_withdraw INTEGER NOT NULL,
            can_deposit INTEGER NOT NULL,
            brokered INTEGER NOT NULL,
            require_self_rade_prevention INTEGER NOT NULL,
            prevent_sor INTEGER NOT NULL,
            update_time INTEGER NOT NULL,
            account_type TEXT NOT NULL,
            uid INTEGER NOT NULL,
            PRIMARY KEY (uid)
        );
        CREATE TABLE IF NOT EXISTS asset_ticker (
            symbol TEXT NOT NULL,
            price_change REAL NOT NULL,
            price_change_percent REAL NOT NULL,
            weighted_average_price REAL NOT NULL,
            first_price REAL NOT NULL,
            last_price REAL NOT NULL,
            last_quantity REAL NOT NULL,
            best_bid_price REAL NOT NULL,
            best_bid_quantity REAL NOT NULL,
            best_ask_price REAL NOT NULL,
            best_ask_quantity REAL NOT NULL,
            open_price REAL NOT NULL,
            high_price REAL NOT NULL,
            low_price REAL NOT NULL,
            total_traded_base_volume REAL NOT NULL,
            total_traded_quote_volume REAL NOT NULL,
            number_of_trades INTEGER NOT NULL,
            PRIMARY KEY (symbol)
        );
        COMMIT;",
        )
        .execute(connection)
        .await
        .map_err(|e| DatabaseError::Initialization(e.to_string()))?;
    }
    Ok(())
}
