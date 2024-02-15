use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
  assets::Pair,
  components::style::{
    button, default_layout, logo, outer_container_block, stylized_block,
  },
  config::{Config, KeyBindings},
  strategy::ModelMetadata,
};
use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
use tokio::sync::mpsc::UnboundedSender;

const SYNC_DURATION: Duration = Duration::milliseconds(500);

#[derive(Default)]
pub struct Models {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_action: usize,
  last_sync: DateTime<Utc>,
}

impl Models {
  pub fn new() -> Self {
    Self::default()
  }

  fn sync_with_fs(&mut self) -> Result<()> {
    if self.last_sync + SYNC_DURATION < Utc::now() {}
    Ok(())
  }
}

impl Screen for Models {
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
          let screen = if self.selected_action == 0 {
            ScreenId::HOME
          } else {
            ScreenId::MODELCONFIG
          };
          command_tx.send(Action::Navigate(screen))?;
        }
      },
      Action::Move(direction) => match direction {
        MoveDirection::Left => {
          self.selected_action = 0;
        },
        MoveDirection::Right => {
          self.selected_action = 1;
        },
        _ => {},
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
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Length(1),
        Constraint::Percentage(30),
        Constraint::Percentage(20),
      ])
      .split(content_layout[1]);

    f.render_widget(button("Back", self.selected_action == 0), button_layout[1]);
    f.render_widget(button("New model", self.selected_action == 1), button_layout[3]);

    Ok(())
  }
}

fn parse_toml_metadata(contents: &str) -> Result<ModelMetadata> {
  let value = contents.parse::<toml::Value>()?;
  let created_at =
    value.get("created").and_then(toml::Value::as_str).unwrap_or_default().to_string();
  let error =
    value.get("error").and_then(toml::Value::as_str).unwrap_or_default().to_string();
  let pair: Pair = value
    .get("created")
    .and_then(toml::Value::as_str)
    .unwrap_or_default()
    .to_string()
    .parse()?;
  let is_finished: bool = value
    .get("created")
    .and_then(toml::Value::as_str)
    .unwrap_or_default()
    .to_string()
    .parse()?;
  Ok(ModelMetadata::new(created_at, pair, is_finished, error))
}
