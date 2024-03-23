use super::{Screen, ScreenId};
use crate::{
  action::{Action, ScreenUpdate},
  assets::Pair,
  components::{
    list::{LabelValueItem, List},
    style::{button, default_layout, outer_container_block, stylized_block},
  },
  config::{Config, KeyBindings},
  core::Command,
  database::{error::DatabaseError, Database},
  statistic::TradingSummary,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use uuid::Uuid;

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
  stats: Option<TradingSummary>,
  core_id: Uuid,
  pair: Pair,
  short_report_list: Option<List<LabelValueItem<String>>>,
}

impl Running {
  pub fn new(core_id: Uuid, pair: Pair) -> Self {
    Self { core_id, pair, ..Self::default() }
  }

  pub fn set_mode(&mut self, mode: RunningMode) {
    self.mode = mode;
  }

  pub fn set_core(&mut self, core_id: Uuid) {
    self.core_id = core_id
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
      Action::Tick => {
        if let Some(command_tx) = &self.command_tx {
          command_tx.send(Action::GenerateRunOverview(self.core_id, self.pair))?;
        }
      },
      Action::ScreenUpdate(update) => match update {
        ScreenUpdate::Running(overview) => {
          if self.short_report_list.is_none() {
            let list = List::default();
            self.short_report_list = Some(list)
          }
          let _ = self.short_report_list.as_mut().is_some_and(|list| {
            list.update_items(overview);
            true
          });
        },
        _ => {},
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          command_tx.send(Action::CoreCommand(Command::Terminate(
            "User finished the run".to_string(),
          )))?;
          command_tx.send(Action::Navigate(ScreenId::REPORT(self.core_id.clone())))?;
        }
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    f.render_widget(outer_container_block(), area);
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)])
      .split(area);
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

    f.render_widget(
      Paragraph::new(format!("Running {}", self.core_id)),
      content_layout[0],
    );

    if let Some(list) = self.short_report_list.as_mut() {
      list.draw(f, content_layout[1])?;
    } else {
      f.render_widget(Paragraph::new("Waiting for DB"), content_layout[1]);
    }
    f.render_widget(button("Finish", true), button_layout[1]);
    Ok(())
  }
}
