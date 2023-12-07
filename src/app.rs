use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
  action::Action,
  components::Component,
  config::Config,
  mode::Mode,
  screens::{home::Home, Screen},
  tui::{self, Tui},
};

pub struct App {
  pub config: Config,
  pub tick_rate: f64,
  pub frame_rate: f64,
  pub screen: Box<dyn Component>,
  pub should_quit: bool,
  pub should_suspend: bool,
  pub mode: Mode,
  action_tx: UnboundedSender<Action>,
  action_rx: UnboundedReceiver<Action>,
  tui: Tui,
}

impl App {
  pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
    let config = Config::new()?;
    let mode = Mode::Home;
    let home = Home::default();
    let (action_tx, action_rx) = mpsc::unbounded_channel();
    let tui = tui::Tui::new()?.tick_rate(tick_rate).frame_rate(frame_rate);

    Ok(Self {
      tick_rate,
      frame_rate,
      screen: Box::new(home),
      should_quit: false,
      should_suspend: false,
      config,
      mode,
      action_tx,
      action_rx,
      tui,
    })
  }

  pub fn mount_screen(&mut self, screen: Screen) -> Result<()> {
    let mut component: Box<dyn Component> = match screen {
      Screen::HOME => Box::new(Home::default()),
      // Screen::RUNS => None,
      // Screen::MODELS => None,
      // Screen::REPORT => None,
    };
    component.register_action_handler(self.action_tx.clone())?;
    component.register_config_handler(self.config.clone())?;
    component.init(self.tui.size()?)?;
    self.screen = component;
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
