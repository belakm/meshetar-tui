use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
  components::style::{
    button, button_style, centered_text, default_header, default_layout, logo, outer_container_block, stylized_block,
  },
  config::{Config, KeyBindings},
};
use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
pub struct RunConfig {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_action: usize,
}

impl RunConfig {
  pub fn new() -> Self {
    Self::default()
  }
}

impl Screen for RunConfig {
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
      Action::Move(direction) => match direction {
        MoveDirection::Left => {
          self.selected_action = self.selected_action.saturating_sub(1);
        },
        MoveDirection::Right => {
          self.selected_action = 1.min(self.selected_action + 1);
        },
        _ => {},
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          let screen_id = if self.selected_action == 0 { ScreenId::BACKTEST } else { ScreenId::RUNNING };
          command_tx.send(Action::Navigate(screen_id))?;
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
    let content_layout =
      Layout::default().constraints(vec![Constraint::Min(0), Constraint::Length(3)]).split(content_area);
    let button_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Length(1),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
      ])
      .split(content_layout[1]);

    f.render_widget(button("BACKTEST", self.selected_action == 0), button_layout[1]);
    f.render_widget(button("RUN", self.selected_action == 1), button_layout[3]);

    Ok(())
  }
}
