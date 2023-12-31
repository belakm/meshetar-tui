const IS_LIVE: bool = false;
const BACKTEST_LAST_N_CANDLES: usize = 1440;
const FETCH_N_DAYS_HISTORY: i64 = 5;
const STARTING_EQUITY: f64 = 1000.0;
const EXCHANGE_FEE: f64 = 0.0;
const DEFAULT_ASSET: Asset = Asset::BTCUSDT;

use chrono::Utc;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::sync::{
  mpsc::{self, UnboundedReceiver, UnboundedSender},
  Mutex,
};
use uuid::Uuid;

use crate::{
  action::{Action, MoveDirection},
  assets::{error::AssetError, Asset, MarketFeed},
  components::style::stylized_block,
  config::Config,
  core::{error::CoreError, Command, Core},
  database::{error::DatabaseError, Database},
  events::EventTx,
  mode::Mode,
  portfolio::{
    allocator::Allocator, error::PortfolioError, risk::RiskEvaluator, Portfolio,
  },
  screens::{
    home::Home,
    models::Models,
    report::Report,
    run_config::RunConfig,
    running::{Running, RunningMode},
    sessions::Sessions,
    Screen, ScreenId,
  },
  statistic::StatisticConfig,
  strategy::Strategy,
  trading::{error::TraderError, execution::Execution, Trader},
  tui::{self, Tui},
  utils::binance_client::{BinanceClient, BinanceClientError},
};

#[derive(Error, Debug)]
enum MainError {
  #[error("Portfolio error: {0}")]
  Portfolio(#[from] PortfolioError),
  #[error("Database error: {0}")]
  Database(#[from] DatabaseError),
  #[error("Core error: {0}")]
  Core(#[from] CoreError),
  #[error("Trader error: {0}")]
  Trader(#[from] TraderError),
  #[error("Binance client error: {0}")]
  BinanceClient(#[from] BinanceClientError),
  #[error("Assets: {0}")]
  Asset(#[from] AssetError),
}

pub struct App {
  pub config: Config,
  pub tick_rate: f64,
  pub frame_rate: f64,
  pub screen: Box<dyn Screen>,
  pub should_quit: bool,
  pub should_suspend: bool,
  pub mode: Mode,
  action_tx: UnboundedSender<Action>,
  action_rx: UnboundedReceiver<Action>,
  database: Arc<Mutex<Database>>,
  portfolio: Arc<Mutex<Portfolio>>,
  core: Core,
  tui: Tui,
}

impl App {
  pub async fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
    let config = Config::new()?;
    let mode = Mode::Home;
    let mut screen = Home::default();
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let tui = tui::Tui::new()?.tick_rate(tick_rate).frame_rate(frame_rate);
    screen.register_action_handler(action_tx.clone())?;
    screen.register_config_handler(config.clone())?;
    screen.init(tui.size()?)?;

    let core_id = Uuid::new_v4();
    let (event_transmitter, event_receiver) = mpsc::unbounded_channel();
    let event_transmitter = EventTx::new(event_transmitter);
    let database: Arc<Mutex<Database>> =
      Arc::new(Mutex::new(Database::new().await.map_err(MainError::from)?));
    let statistic_config = StatisticConfig {
      starting_equity: STARTING_EQUITY,
      trading_days_per_year: 365,
      risk_free_return: 0.0,
      created_at: Utc::now(),
    };
    let portfolio: Arc<Mutex<Portfolio>> = Arc::new(Mutex::new(
      Portfolio::builder()
        .database(database.clone())
        .core_id(core_id.clone())
        .allocation_manager(Allocator { default_order_value: 100.0 })
        .risk_manager(RiskEvaluator {})
        .starting_cash(STARTING_EQUITY)
        .assets(vec![DEFAULT_ASSET])
        .statistic_config(statistic_config.clone())
        .build()
        .await?,
    ));

    let mut traders = Vec::new();
    let (command_transmitter, command_receiver) = mpsc::channel::<Command>(20);
    let (trader_command_transmitter, trader_command_receiver) =
      mpsc::channel::<Command>(20);
    let command_transmitters =
      HashMap::from([(DEFAULT_ASSET, trader_command_transmitter)]);
    traders.push(
      Trader::builder()
        .core_id(core_id)
        .asset(DEFAULT_ASSET)
        .trading_is_live(IS_LIVE)
        .command_reciever(trader_command_receiver)
        .event_transmitter(event_transmitter)
        .portfolio(Arc::clone(&portfolio))
        .market_feed(MarketFeed::new(
          DEFAULT_ASSET,
          IS_LIVE,
          database.clone(),
          BACKTEST_LAST_N_CANDLES,
        ))
        .strategy(Strategy::new(DEFAULT_ASSET))
        .execution(Execution::new(EXCHANGE_FEE))
        .build()?,
    );

    let core = Core::builder()
      .id(core_id)
      .binance_client(BinanceClient::new().await.map_err(MainError::from)?)
      .portfolio(portfolio.clone())
      .command_reciever(command_receiver)
      .command_transmitters(command_transmitters)
      .traders(traders)
      .database(database.clone())
      .statistics_config(statistic_config)
      .n_days_history_fetch(FETCH_N_DAYS_HISTORY)
      .build()?;

    Ok(Self {
      tick_rate,
      frame_rate,
      screen: Box::new(screen),
      should_quit: false,
      should_suspend: false,
      config,
      mode,
      action_tx,
      action_rx,
      tui,
      database,
      portfolio,
      core,
    })
  }

  pub fn navigate(&mut self, screen: ScreenId) -> Result<()> {
    let mut screen: Box<dyn Screen> = match screen {
      ScreenId::HOME => Box::new(Home::default()),
      ScreenId::SESSIONS => Box::new(Sessions::default()),
      ScreenId::MODELS => Box::new(Models::default()),
      ScreenId::REPORT => Box::new(Report::default()),
      ScreenId::RUNNING => {
        let mut running = Running::default();
        running.set_mode(RunningMode::RUNNING);
        Box::new(running)
      },
      ScreenId::BACKTEST => Box::new(Running::default()),
      ScreenId::RUNCONFIG => Box::new(RunConfig::default()),
    };
    screen.register_action_handler(self.action_tx.clone())?;
    screen.register_config_handler(self.config.clone())?;
    screen.init(self.tui.size()?)?;
    self.screen = screen;
    Ok(())
  }

  pub async fn run(&mut self) -> Result<()> {
    self.tui.enter()?;
    let action_tx = self.action_tx.clone();
    loop {
      if let Some(e) = self.tui.next().await {
        match e {
          tui::Event::Quit => action_tx.send(Action::Quit)?,
          tui::Event::Tick => action_tx.send(Action::Tick)?,
          tui::Event::Render => action_tx.send(Action::Render)?,
          tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
          tui::Event::Key(key) => {
            if let Some(keymap) = self.config.keybindings.get(&self.mode) {
              if let Some(action) = keymap.get(&vec![key]) {
                log::info!("Got action: {action:?}");
                action_tx.send(action.clone())?;
              }
            };
            match key.code {
              KeyCode::Up => {
                let _ = action_tx.send(Action::Move(MoveDirection::Up));
              },
              KeyCode::Down => {
                let _ = action_tx.send(Action::Move(MoveDirection::Down));
              },
              KeyCode::Left => {
                let _ = action_tx.send(Action::Move(MoveDirection::Left));
              },
              KeyCode::Right => {
                let _ = action_tx.send(Action::Move(MoveDirection::Right));
              },
              KeyCode::Enter => {
                let _ = action_tx.send(Action::Accept);
              },
              KeyCode::Esc => {
                let _ = action_tx.send(Action::Navigate(ScreenId::HOME));
              },
              KeyCode::Char('q') => {
                let _ = action_tx.send(Action::Quit);
              },
              _ => {},
            }
          },
          _ => {},
        }
        if let Some(action) = self.screen.handle_events(Some(e.clone()))? {
          action_tx.send(action)?;
        }
      }

      while let Ok(action) = self.action_rx.try_recv() {
        log::debug!("{action:?}");
        match action {
          Action::Tick => {},
          Action::Quit => self.should_quit = true,
          Action::Suspend => self.should_suspend = true,
          Action::Resume => self.should_suspend = false,
          Action::Resize(w, h) => {
            self.tui.resize(Rect::new(0, 0, w, h))?;
            self.tui.draw(|f| {
              let r = self.screen.draw(f, f.size());
              if let Err(e) = r {
                action_tx
                  .send(Action::Error(format!("Failed to draw: {:?}", e)))
                  .unwrap();
              }
            })?;
          },
          Action::Render => {
            self.tui.draw(|f| {
              let r = self.screen.draw(f, f.size());
              if let Err(e) = r {
                action_tx
                  .send(Action::Error(format!("Failed to draw: {:?}", e)))
                  .unwrap();
              }
            })?;
          },
          Action::Navigate(screen) => {
            self.navigate(screen)?;
          },
          _ => {},
        }
        if let Some(action) = self.screen.update(action.clone())? {
          action_tx.send(action)?
        };
      }
      if self.should_suspend {
        self.tui.suspend()?;
        action_tx.send(Action::Resume)?;
        self.tui = tui::Tui::new()?.tick_rate(self.tick_rate).frame_rate(self.frame_rate);
        // tui.mouse(true);
        self.tui.enter()?;
      } else if self.should_quit {
        self.tui.stop()?;
        break;
      }
    }
    self.tui.exit()?;
    Ok(())
  }
}
