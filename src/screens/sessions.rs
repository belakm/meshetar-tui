use super::{Screen, ScreenId};
use crate::{
  action::Action,
  components::style::{
    button, default_layout, logo, outer_container_block, stylized_block,
  },
  config::{Config, KeyBindings},
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

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
      Action::Tick => {},
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
    f.render_widget(outer_container_block(), area);
    let inner_area = area.inner(&Margin { horizontal: 2, vertical: 2 });
    let (header_area, content_area) = default_layout(inner_area);
    f.render_widget(logo(), header_area);
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Min(0), Constraint::Length(3)])
      .split(content_area);
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
