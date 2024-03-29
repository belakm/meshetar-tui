use super::{Screen, ScreenId};
use crate::{
  action::Action,
  components::style::{button, default_layout, outer_container_block, stylized_block},
  config::{Config, KeyBindings},
};
use crossterm::event::{KeyCode, KeyEvent};
use eyre::Result;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Default)]
pub struct Sessions {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
}

impl Sessions {
  pub fn new() -> Self {
    Self::default()
  }
}

impl Screen for Sessions {
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
        // Get stats
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          command_tx.send(Action::Navigate(ScreenId::HOME))?;
        }
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Min(0), Constraint::Length(3)])
      .split(area);
    let button_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
      ])
      .split(content_layout[1]);
    f.render_widget(Paragraph::new("TODO: List of sessions"), content_layout[0]);
    f.render_widget(button("Back", true), button_layout[1]);
    Ok(())
  }
}
