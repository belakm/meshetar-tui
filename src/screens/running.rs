use super::{Screen, ScreenId};
use crate::{
  action::Action,
  assets::Pair,
  components::style::{
    button, default_layout, logo, outer_container_block, stylized_block,
  },
  config::{Config, KeyBindings},
  core::Command,
  database::Database,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{mpsc::UnboundedSender, Mutex};

#[derive(Default)]
pub enum RunningMode {
  #[default]
  BACKTEST,
  RUNNING,
}

#[derive(Default)]
pub struct Running {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  mode: RunningMode,
  database: Option<Arc<Mutex<Database>>>,
  pair: Pair,
}

impl Running {
  pub fn new(database: Arc<Mutex<Database>>, pair: Pair) -> Self {
    Self { database: Some(database), pair, ..Self::default() }
  }

  pub fn set_mode(&mut self, mode: RunningMode) {
    self.mode = mode;
  }
}

impl Screen for Running {
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
    self.command_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(&mut self, config: Config) -> Result<()> {
    self.config = config;
    Ok(())
  }

  fn update(&mut self, action: Action) -> Result<Option<Action>> {
    match action {
      Action::Tick => {},
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          command_tx.send(Action::CoreCommand(Command::Terminate(
            "User finished the run".to_string(),
          )))?;
          command_tx.send(Action::Navigate(ScreenId::REPORT))?;
        }
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let stats = {
      if let Some(db) = self.database.clone() {
        let mut db = db.try_lock()?;
        let stats = db.get_statistics(&self.pair.clone());
        Some(stats)
      } else {
        None
      }
    };

    f.render_widget(outer_container_block(), area);
    let inner_area = area.inner(&Margin { horizontal: 2, vertical: 2 });
    let (header_area, content_area) = default_layout(inner_area);
    f.render_widget(logo(), header_area);
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)])
      .split(content_area);
    let button_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
      ])
      .split(content_layout[2]);

    // Balance
    // Trades
    // Change

    f.render_widget(Paragraph::new("Running :)"), content_layout[0]);
    if let Some(Ok(stats)) = stats {
      f.render_widget(
        Paragraph::new(format!("{}", stats.pnl.total_pnl)),
        content_layout[1],
      );
    } else {
      f.render_widget(Paragraph::new("Waiting for DB"), content_layout[1]);
    }
    f.render_widget(button("Finish", true), button_layout[1]);
    Ok(())
  }
}
