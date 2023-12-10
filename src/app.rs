const IS_LIVE: bool = false;
const BACKTEST_LAST_N_CANDLES: usize = 1440;
const FETCH_N_DAYS_HISTORY: i64 = 5;
const STARTING_EQUITY: f64 = 1000.0;
const EXCHANGE_FEE: f64 = 0.0;
const DEFAULT_ASSET: Asset = Asset::BTCUSDT;

use std::sync::Arc;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc::{self, UnboundedReceiver, UnboundedSender}, Mutex};

use crate::{
  action::{Action, MoveDirection},
  components::style::stylized_block,
  config::Config,
  mode::Mode,
  screens::{
    home::Home,
    models::Models,
    report::Report,
    run_config::RunConfig,
    running::{Running, RunningMode},
    sessions::Sessions,
    Screen, ScreenId,
  },
  tui::{self, Tui},
};

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
  database: Arc<Mutex<Database>>
  tui: Tui,
}

impl App {
  pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
    let config = Config::new()?;
    let mode = Mode::Home;
    let mut screen = Home::default();
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let tui = tui::Tui::new()?.tick_rate(tick_rate).frame_rate(frame_rate);
    screen.register_action_handler(action_tx.clone())?;
    screen.register_config_handler(config.clone())?;
    screen.init(tui.size()?)?;
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

    let core = 

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
                action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
              }
            })?;
          },
          Action::Render => {
            self.tui.draw(|f| {
              let r = self.screen.draw(f, f.size());
              if let Err(e) = r {
                action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
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
