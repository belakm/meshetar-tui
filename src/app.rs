use crate::{
  action::{Action, MoveDirection, ScreenUpdate},
  assets::{asset_ticker, error::AssetError, MarketEvent, MarketFeed, Pair},
  components::{
    header::MeshetarHeader,
    style::{outer_container_block, stylized_block},
  },
  config::Config,
  core::{error::CoreError, Command, Core, CoreMessage},
  database::{error::DatabaseError, Database},
  events::{Event, EventTx},
  exchange::{
    account::{get_account_from_exchange, new_account_stream, ExchangeAccount},
    binance_client::{self, BinanceClient, BinanceClientError},
    error::ExchangeError,
    ExchangeEvent,
  },
  mode::Mode,
  portfolio::{
    allocator::Allocator, error::PortfolioError, risk::RiskEvaluator, Portfolio,
  },
  screens::{
    exchange::Exchange,
    home::Home,
    model_config::ModelConfig,
    models::Models,
    report::Report,
    run_config::{CoreConfiguration, RunConfig},
    running::{Running, RunningMode},
    sessions::Sessions,
    Screen, ScreenId,
  },
  statistic::{StatisticConfig, TradingSummary},
  strategy::{generate_new_model, Strategy},
  trading::{error::TraderError, execution::Execution, Trader},
  tui::{self, Frame, Tui},
  utils::load_config::{self, read_config, ExchangeConfig},
};
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use eyre::Result;
use ratatui::{
  layout::{Constraint, Layout, Margin},
  prelude::Rect,
  widgets::Clear,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{
  broadcast,
  mpsc::{self, error::TryRecvError, UnboundedReceiver, UnboundedSender},
  Mutex,
};
use uuid::Uuid;

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
  #[error("Exchange: {0}")]
  Exchange(#[from] ExchangeError),
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
  event_broadcast: broadcast::Sender<Event>,
  database: Arc<Mutex<Database>>,
  portfolio: Arc<Mutex<Portfolio>>,
  core: Option<Core>,
  core_command_tx: Option<mpsc::Sender<Command>>,
  binance_client: BinanceClient,
  tui: Tui,
  use_testnet: bool,
  header: MeshetarHeader,
}

static STATISTIC_CONFIG: StatisticConfig = StatisticConfig {
  starting_equity: 0f64,
  trading_days_per_year: 365,
  risk_free_return: 0.0,
  created_at: DateTime::UNIX_EPOCH,
};

impl App {
  async fn new_run(
    &mut self,
    core_configuration: CoreConfiguration,
  ) -> Result<(Uuid, Pair)> {
    let mut traders = Vec::new();
    let core_id = Uuid::new_v4();
    let pair = core_configuration.pair.clone();
    let (event_transmitter, event_receiver) = mpsc::unbounded_channel();
    let event_transmitter = EventTx::new(event_transmitter);
    let (core_command_tx, core_command_rx) = mpsc::channel::<Command>(20);
    let (core_message_tx, mut core_message_rx) = mpsc::channel::<CoreMessage>(20);
    let (trader_command_transmitter, trader_command_receiver) =
      mpsc::channel::<Command>(20);
    let command_transmitters =
      HashMap::from([(core_configuration.pair, trader_command_transmitter)]);
    let event_rx = self.event_broadcast.subscribe();

    let trader_client = self.binance_client.clone();
    traders.push(
      Trader::builder()
        .core_id(core_id)
        .pair(core_configuration.pair)
        .trading_is_live(core_configuration.run_live)
        .command_reciever(trader_command_receiver)
        .event_transmitter(event_transmitter)
        .portfolio(Arc::clone(&self.portfolio))
        .strategy(Strategy::new(core_configuration.pair, core_configuration.model_name))
        .execution(Execution::new(core_configuration.exchange_fee, trader_client))
        .event_rx(event_rx)
        .build()?,
    );

    let statistic_config = StatisticConfig {
      starting_equity: core_configuration.starting_equity,
      created_at: Utc::now(),
      ..STATISTIC_CONFIG
    };

    let mut core = Core::builder()
      .id(core_id)
      .binance_client(self.binance_client.clone())
      .portfolio(self.portfolio.clone())
      .command_rx(core_command_rx)
      .message_tx(core_message_tx)
      .command_transmitters(command_transmitters)
      .traders(traders)
      .database(self.database.clone())
      .statistics_config(statistic_config)
      .n_days_history_fetch(core_configuration.n_days_to_fetch as i64)
      .is_backtest(!core_configuration.run_live)
      .build()?;

    self.core_command_tx = Some(core_command_tx);

    // This forwards messages from Core to App
    let action_tx_clone = self.action_tx.clone();
    tokio::spawn(async move {
      loop {
        match core_message_rx.try_recv() {
          Ok(msg) => {
            let _ = action_tx_clone.send(Action::CoreMessage(msg));
          },
          Err(e) => match e {
            TryRecvError::Disconnected => {
              break;
            },
            TryRecvError::Empty => {},
          },
        }
      }
    });

    // This starts the Core and sends message when it ends
    let action_tx = self.action_tx.clone();
    tokio::spawn(async move {
      match core.run().await {
        Ok(_) => log::info!("Core {} finished.", core_id),
        Err(e) => log::error!("{}", e.to_string()),
      };
      let _ = action_tx.send(Action::CoreMessage(CoreMessage::Finished(core_id)));
    });

    Ok((core_id, pair))
  }

  pub async fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
    let config = Config::new()?;
    let mode = Mode::Home;
    let mut screen = Home::default();
    let tui = tui::Tui::new()?.tick_rate(tick_rate).frame_rate(frame_rate);
    let use_testnet = read_config()?.use_testnet;
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let (event_broadcast, mut event_rx) = broadcast::channel(20);
    let binance_client = BinanceClient::new().await.map_err(MainError::from)?;
    let binance_client_clone = binance_client.clone();
    let pairs = vec![Pair::BTCUSDT, Pair::ETHBTC];
    let database: Arc<Mutex<Database>> =
      Arc::new(Mutex::new(Database::new().await.map_err(MainError::from)?));
    let portfolio: Arc<Mutex<Portfolio>> = Arc::new(Mutex::new(
      Portfolio::builder()
        .database(database.clone())
        .allocation_manager(Allocator { default_order_value: 100.0 })
        .risk_manager(RiskEvaluator {})
        .statistic_config(STATISTIC_CONFIG)
        .build()
        .await?,
    ));

    screen.register_action_handler(action_tx.clone())?;
    screen.register_config_handler(config.clone())?;
    screen.init(tui.size()?)?;

    let binance_client_clone = binance_client.clone();
    let event_tx = event_broadcast.clone();
    tokio::spawn(async move {
      let stream_url = ExchangeConfig::get_exchange_stream_url(use_testnet);
      let binance_client_for_account = binance_client_clone.clone();
      log::info!("Fething initial balances.");
      match get_account_from_exchange(binance_client_for_account).await {
        Ok(account) => {
          if let Err(e) =
            event_tx.send(Event::Exchange(ExchangeEvent::ExchangeAccount(account)))
          {
            log::warn!("Error sending account update.");
          }
        },
        Err(e) => {
          log::error!("{:?}", e);
          return;
        },
      }
      // GET CRYPTO TICKER
      match asset_ticker::new_ticker(pairs, &stream_url).await {
        Ok(mut ticker) => {
          // GET ACCOUNT LISTENER
          match new_account_stream(&stream_url, binance_client_clone).await {
            Ok(mut account_listener) => {
              log::info!("Database loop started.");
              loop {
                match ticker.try_recv() {
                  Ok(market_event) => {
                    if let Err(e) = event_tx.send(Event::Market(market_event)) {
                      log::warn!("Error sending market event.");
                    }
                  },
                  Err(e) => match e {
                    mpsc::error::TryRecvError::Empty => {},
                    mpsc::error::TryRecvError::Disconnected => {
                      log::info!("Asset ticker disconnected.");
                      return;
                    },
                  },
                }
                match account_listener.try_recv() {
                  Ok(balances) => {
                    if let Err(e) = event_tx.send(Event::Exchange(
                      ExchangeEvent::ExchangeBalanceUpdate(balances),
                    )) {
                      log::warn!("Error sending account balance update");
                    }
                  },
                  Err(e) => match e {
                    mpsc::error::TryRecvError::Empty => continue,
                    mpsc::error::TryRecvError::Disconnected => {
                      log::info!("Account listener disconnected.");
                      return;
                    },
                  },
                }
              }
            },
            Err(e) => {
              log::error!("{:?}", e);
              return;
            },
          }
        },
        Err(e) => {
          log::error!("{:?}", e);
          return;
        },
      };
    });

    let db_clone = database.clone();
    let event_tx = event_broadcast.clone();
    tokio::spawn(async move {
      loop {
        match event_rx.try_recv() {
          Ok(event) => match event {
            Event::Exchange(exchange_event) => match exchange_event {
              ExchangeEvent::ExchangeAccount(account) => {
                let lock = db_clone.lock();
                lock.await.set_exchange_account(account);
              },
              ExchangeEvent::ExchangeBalanceUpdate(balances) => {
                let lock = db_clone.lock();
                lock.await.set_exchange_balances(balances);
              },
              ExchangeEvent::Market(market_event) => {
                if let Err(e) = event_tx.send(Event::Market(market_event)) {
                  log::warn!("Error passing on event market update");
                }
              },
            },
            _ => {},
          },
          Err(e) => match e {
            broadcast::error::TryRecvError::Lagged(n) => {
              log::warn!("Event broadcast lagging behind {} events.", n);
            },
            broadcast::error::TryRecvError::Empty => {},
            broadcast::error::TryRecvError::Closed => {
              return;
            },
          },
        }
      }
    });

    Ok(Self {
      use_testnet,
      tick_rate,
      frame_rate,
      screen: Box::new(screen),
      should_quit: false,
      should_suspend: false,
      config,
      mode,
      action_tx,
      action_rx,
      event_broadcast,
      tui,
      database,
      portfolio,
      core: None,
      binance_client,
      core_command_tx: None,
      header: MeshetarHeader::new(use_testnet),
    })
  }

  pub fn navigate(&mut self, screen: ScreenId) -> Result<()> {
    let mut screen: Box<dyn Screen> = match screen {
      ScreenId::HOME => Box::new(Home::default()),
      ScreenId::SESSIONS => Box::new(Sessions::default()),
      ScreenId::MODELS => Box::new(Models::default()),
      ScreenId::MODELCONFIG => Box::new(ModelConfig::default()),
      ScreenId::REPORT(core_id) => {
        let screen = Box::new(Report::new(core_id));
        self.action_tx.send(Action::GenerateReport(core_id))?;
        screen
      },
      ScreenId::RUNNING((core_id, pair)) => {
        let mut running = Running::new(core_id, pair);
        running.set_mode(RunningMode::RUNNING);
        Box::new(running)
      },
      ScreenId::RUNCONFIG => Box::new(RunConfig::new()),
      ScreenId::EXCHANGE => Box::new(Exchange::new()),
    };
    screen.register_action_handler(self.action_tx.clone())?;
    screen.register_config_handler(self.config.clone())?;
    screen.init(self.tui.size()?)?;
    self.screen = screen;
    Ok(())
  }

  fn draw(&mut self) -> Result<()> {
    self.tui.draw(|f| {
      let area = f.size();
      f.render_widget(outer_container_block(), area);
      let layout = Layout::vertical(vec![
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
      ])
      .split(area.inner(&Margin { horizontal: 1, vertical: 1 }));
      if let Err(e) = self.header.draw(f, layout[0]) {
        let action_tx = self.action_tx.clone();
        action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
      }
      if let Err(e) = self.screen.draw(f, layout[2]) {
        let action_tx = self.action_tx.clone();
        action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
      }
    })?;
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
        let action_clone = action.clone();
        let action_clone_log = action.clone();

        if action_clone_log != Action::Tick && action_clone_log != Action::Render {
          log::debug!("{action:?}");
        }

        match action {
          Action::Tick => {
            let header_last_updated =
              self.header.last_updated().unwrap_or(DateTime::default());
            if Utc::now() - Duration::from_secs(10) > header_last_updated {
              let db = self.database.lock().await;
              let valuation = db.get_valuation();
              drop(db);
              self.header.update(valuation.0, valuation.1);
            }
          },
          Action::Quit => self.should_quit = true,
          Action::Suspend => self.should_suspend = true,
          Action::Resume => self.should_suspend = false,
          Action::Resize(w, h) => {
            self.tui.resize(Rect::new(0, 0, w, h))?;
            self.draw()?;
          },
          Action::Render => {
            self.draw()?;
          },
          Action::Navigate(screen) => {
            self.navigate(screen)?;
          },
          Action::CoreCommand(command) => match command {
            Command::Start(core_configuration) => {
              let (core_id, pair) = self.new_run(core_configuration).await?;
              let _ = self.navigate(ScreenId::RUNNING((core_id, pair)))?;
            },
            _ => {
              if let Some(tx) = &self.core_command_tx {
                tx.send(command).await?;
              }
            },
          },
          Action::CoreMessage(msg) => match msg {
            CoreMessage::Finished(core_id) => {
              self.navigate(ScreenId::REPORT(core_id))?;
            },
          },

          Action::GenerateModel(pair) => {
            log::warn!("Starting new model generation");
            tokio::spawn(async move {
              match generate_new_model(pair).await {
                Ok(_) => {
                  log::warn!("New model created.");
                },
                Err(e) => {
                  log::error!("Error on new model creation. {}", e);
                },
              }
            });
          },
          Action::GenerateRunOverview(core_id, pair) => {
            let mut db = self.database.try_lock()?;
            if let Ok(report) = db.generate_run_overview(&core_id, &pair) {
              action_tx.send(Action::ScreenUpdate(ScreenUpdate::Running(report)))?;
            }
          },
          Action::GenerateReport(core_id) => {
            let mut db = self.database.try_lock()?;
            if let Ok(report) = db.get_statistics(&core_id) {
              action_tx.send(Action::ScreenUpdate(ScreenUpdate::Report(report)))?;
            }
          },
          _ => {},
        }
        if let Some(action) = self.screen.update(action_clone.clone())? {
          action_tx.send(action)?
        };
      }
      if self.should_suspend {
        self.tui.suspend()?;
        action_tx.send(Action::Resume)?;
        self.tui = tui::Tui::new()?.tick_rate(self.tick_rate).frame_rate(self.frame_rate);
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
