pub mod error;

use crate::{
  assets::{fetch_candles, Pair},
  database::Database,
  portfolio::Portfolio,
  screens::run_config::CoreConfiguration,
  statistic::{StatisticConfig, TradingSummary},
  trading::Trader,
  utils::binance_client::BinanceClient,
};
use chrono::Duration;
use error::CoreError;
use prettytable::Table;
use serde::Serialize;
use std::{collections::HashMap, fs::File, io::Write, sync::Arc};
use tokio::sync::{
  mpsc::{self, Receiver, Sender},
  Mutex,
};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Serialize, Clone, PartialEq, Debug)]
pub enum Command {
  ExitPosition(Pair),
  ExitAllPositions,
  Terminate(String),
  Start(CoreConfiguration),
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub enum CoreMessage {
  Finished,
}

pub struct Core {
  id: Uuid,
  database: Arc<Mutex<Database>>,
  portfolio: Arc<Mutex<Portfolio>>,
  binance_client: Arc<BinanceClient>,
  pub command_rx: Receiver<Command>,
  message_tx: Sender<CoreMessage>,
  command_transmitters: HashMap<Pair, mpsc::Sender<Command>>,
  statistics_config: StatisticConfig,
  traders: Vec<Trader>,
  n_days_history_fetch: i64,
}

impl Core {
  pub fn builder() -> CoreBuilder {
    CoreBuilder::new()
  }
}

impl Core {
  pub async fn run(&mut self) -> Result<(), CoreError> {
    info!("Core {} is starting up.", &self.id);
    if self.n_days_history_fetch > 0 {
      let mut fetching_stopped = self.fetch_history(self.n_days_history_fetch).await;
      loop {
        tokio::select! {
            _ = fetching_stopped.recv() => {
                let _ = self
                .portfolio
                .lock()
                .await
                .init_core_in_db(self.id, self.statistics_config.starting_equity)
                .await;
                break;
            }
        }
      }
    }
    let mut trading_stopped = self.run_traders().await;
    loop {
      tokio::select! {
          _ = trading_stopped.recv() => {
              break;
          },
          command = self.command_rx.recv() => {
              if let Some(command) = command {
                  match command {
                      Command::ExitPosition(asset) => {
                          self.exit_position(asset).await;
                      }
                      Command::ExitAllPositions => {
                          self.exit_all_positions().await;
                      }
                      Command::Terminate(message) => {
                          self.terminate_traders(message).await;
                          break;
                      },
                      _  => {}
                  }
              } else {
                  break;
              }
          }
      }
    }

    // File to print out the statistics
    if let Ok(mut out) = File::create("summary.html") {
      let css_content = std::fs::read_to_string("summary.css").unwrap();
      writeln!(out, "<style>{}</style>", css_content).unwrap();
      let (overall_stats_tables, exited_trades_table) =
        self.generate_session_summary().await?;
      overall_stats_tables.iter().for_each(|table| {
        let _ = table.print_html(&mut out);
        // let _ = table.printstd();
      });
      let _ = exited_trades_table.print_html(&mut out);
      warn!("\n\n\nCheck summary.html for backtesting stats\n\n");
    }

    Ok(())
  }

  async fn fetch_history(&mut self, n_days: i64) -> mpsc::Receiver<bool> {
    let assets: Vec<Pair> =
      self.traders.iter().map(|trader| trader.pair.clone()).collect();
    let binance_client = self.binance_client.clone();
    let handles = assets.into_iter().map(move |asset| {
      (
        asset.clone(),
        fetch_candles(Duration::days(n_days), asset.clone(), binance_client.clone()),
      )
    });
    let (notify_transmitter, notify_receiver) = mpsc::channel(1);
    let database = self.database.clone();
    tokio::spawn(async move {
      for handle in handles {
        match handle.1.await {
          Ok(candles) => {
            let _ = database.lock().await.add_candles(handle.0, candles).await;
          },
          Err(err) => {
            error!(
              error = &*format!("{:?}", err),
              "Trader thread has panicked during execution",
            )
          },
        }
      }
      let _ = notify_transmitter.send(true).await;
    });
    notify_receiver
  }
  async fn run_traders(&mut self) -> mpsc::Receiver<bool> {
    let traders = std::mem::take(&mut self.traders);
    let mut thread_handles = Vec::with_capacity(traders.len());
    for mut trader in traders.into_iter() {
      let handle = tokio::spawn(async move { trader.run().await });
      thread_handles.push(handle);
    }
    let (notify_transmitter, notify_receiver) = mpsc::channel(1);
    tokio::spawn(async move {
      for handle in thread_handles {
        if let Err(err) = handle.await {
          error!(
            error = &*format!("{:?}", err),
            "Trader thread has panicked during execution",
          )
        }
      }
      let _ = notify_transmitter.send(true).await;
    });
    notify_receiver
  }
  async fn terminate_traders(&self, message: String) {
    self.exit_all_positions().await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    for (market, command_transmitter) in self.command_transmitters.iter() {
      if command_transmitter.send(Command::Terminate(message.clone())).await.is_err() {
        error!(why = "dropped receiver", asset = &*format!("{:?}", market),);
      }
    }
  }
  async fn exit_all_positions(&self) {
    for (asset, command_transmitter) in self.command_transmitters.iter() {
      if command_transmitter.send(Command::ExitPosition(asset.clone())).await.is_err() {
        error!(
          asset = &*format!("{:?}", asset),
          why = "dropped receiver",
          "failed to send Command::Terminate to Trader command_rx"
        );
      }
    }
  }
  async fn exit_position(&self, asset: Pair) {
    if let Some((market_ref, command_tx)) =
      self.command_transmitters.get_key_value(&asset)
    {
      if command_tx.send(Command::ExitPosition(asset)).await.is_err() {
        error!(
          market = &*format!("{:?}", market_ref),
          why = "dropped receiver",
          "failed to send Command::Terminate to Trader command_rx"
        );
      }
    } else {
      warn!(
        market = &*format!("{:?}", asset),
        why = "Engine has no trader_command_tx associated with provided Market",
        "failed to exit Position"
      );
    }
  }
  async fn generate_session_summary(&self) -> Result<(Vec<Table>, Table), CoreError> {
    // Fetch statistics for each Market

    let assets: Vec<_> = self.command_transmitters.clone().into_keys().collect();
    let mut stats_per_market = Vec::new();
    let futures: Vec<_> = assets
      .into_iter()
      .map(|asset| {
        let portfolio_clone = self.portfolio.clone();
        tokio::spawn(async move {
          let mut portfolio = portfolio_clone.lock().await;
          match portfolio.get_statistics(&asset).await {
            Ok(statistics) => Some((asset, statistics)),
            Err(error) => {
              error!(
                ?error,
                ?asset,
                "failed to get Market statistics when generating trading session summary"
              );
              None
            },
          }
        })
      })
      .collect();

    for future in futures {
      if let Some(result) = future.await.unwrap() {
        stats_per_market.push(result);
      }
    }

    let mut database = self.database.lock().await;
    let final_balance = database.get_balance(self.id).ok();
    let min_start_time = stats_per_market
      .iter()
      .map(|(_, stats)| stats)
      .min_by(|stats1, stats2| stats1.starting_time.cmp(&stats2.starting_time))
      .map(|stats| stats.starting_time)
      .to_owned()
      .unwrap();
    warn!("TRADING SINCE: {}", min_start_time);
    let mut statistics_summary =
      TradingSummary::init(self.statistics_config, Some(min_start_time));
    warn!("FINAL BALANCE: {:?}", final_balance);

    // Generate average statistics across all markets using session's exited Positions
    let exited_positions = database.get_exited_positions(self.id)?;
    statistics_summary.generate_summary(&exited_positions);
    let exited_positions_table =
      crate::statistic::exited_positions_table(exited_positions);
    let stats_per_market: Vec<_> = stats_per_market
      .into_iter()
      .map(|(asset, summary)| (asset.to_string(), summary))
      .collect();

    let overall_stats_tables = crate::statistic::combine(
      stats_per_market
        .into_iter()
        .chain([("Total".to_owned(), statistics_summary)])
        .collect(),
    );

    Ok((overall_stats_tables, exited_positions_table))
  }
}

pub struct CoreBuilder {
  id: Option<Uuid>,
  portfolio: Option<Arc<Mutex<Portfolio>>>,
  database: Option<Arc<Mutex<Database>>>,
  binance_client: Option<BinanceClient>,
  command_rx: Option<Receiver<Command>>,
  message_tx: Option<Sender<CoreMessage>>,
  command_transmitters: Option<HashMap<Pair, mpsc::Sender<Command>>>,
  traders: Option<Vec<Trader>>,
  statistics_config: Option<StatisticConfig>,
  n_days_history_fetch: Option<i64>,
}

impl CoreBuilder {
  pub fn new() -> Self {
    CoreBuilder {
      id: None,
      database: None,
      portfolio: None,
      binance_client: None,
      message_tx: None,
      command_rx: None,
      command_transmitters: None,
      traders: None,
      statistics_config: None,
      n_days_history_fetch: None,
    }
  }
  pub fn id(self, id: Uuid) -> Self {
    CoreBuilder { id: Some(id), ..self }
  }
  pub fn portfolio(self, portfolio: Arc<Mutex<Portfolio>>) -> Self {
    CoreBuilder { portfolio: Some(portfolio), ..self }
  }
  pub fn binance_client(self, binance_client: BinanceClient) -> Self {
    CoreBuilder { binance_client: Some(binance_client), ..self }
  }
  pub fn command_rx(self, command_reciever: Receiver<Command>) -> Self {
    CoreBuilder { command_rx: Some(command_reciever), ..self }
  }
  pub fn message_tx(self, command_reciever: Sender<CoreMessage>) -> Self {
    CoreBuilder { message_tx: Some(command_reciever), ..self }
  }
  pub fn command_transmitters(self, value: HashMap<Pair, mpsc::Sender<Command>>) -> Self {
    CoreBuilder { command_transmitters: Some(value), ..self }
  }
  pub fn database(self, value: Arc<Mutex<Database>>) -> Self {
    CoreBuilder { database: Some(value), ..self }
  }
  pub fn traders(self, value: Vec<Trader>) -> Self {
    CoreBuilder { traders: Some(value), ..self }
  }
  pub fn statistics_config(self, value: StatisticConfig) -> Self {
    CoreBuilder { statistics_config: Some(value), ..self }
  }
  pub fn n_days_history_fetch(self, value: i64) -> Self {
    CoreBuilder { n_days_history_fetch: Some(value), ..self }
  }
  pub fn build(self) -> Result<Core, CoreError> {
    let binance_client =
      self.binance_client.ok_or(CoreError::BuilderIncomplete("binance client"))?;
    let binance_client = Arc::new(binance_client);
    let core = Core {
      id: self.id.ok_or(CoreError::BuilderIncomplete("core_id"))?,
      database: self.database.ok_or(CoreError::BuilderIncomplete("database"))?,
      portfolio: self.portfolio.ok_or(CoreError::BuilderIncomplete("portfolio"))?,
      binance_client,
      message_tx: self
        .message_tx
        .ok_or(CoreError::BuilderIncomplete("command reciever"))?,
      command_rx: self
        .command_rx
        .ok_or(CoreError::BuilderIncomplete("command reciever"))?,
      command_transmitters: self
        .command_transmitters
        .ok_or(CoreError::BuilderIncomplete("trader command transmitters"))?,
      traders: self.traders.ok_or(CoreError::BuilderIncomplete("traders"))?,
      statistics_config: self
        .statistics_config
        .ok_or(CoreError::BuilderIncomplete("statistics summary"))?,
      n_days_history_fetch: self
        .n_days_history_fetch
        .ok_or(CoreError::BuilderIncomplete("n_days_history_fetch"))?,
    };
    Ok(core)
  }
}
