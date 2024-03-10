use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
  components::style::{
    default_layout, header_style, logo, outer_container_block, stylized_block,
    stylized_button,
  },
  config::{Config, KeyBindings},
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{collections::HashMap, time::Duration};
use strum::{Display, EnumCount, EnumIter, EnumString, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
pub struct Home {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_action: usize,
}

#[derive(EnumIter, EnumString, EnumCount, Display)]
enum HomeAction {
  RUN,
  MODELS,
  SESSIONS,
}
impl HomeAction {
  fn to_screen_id(&self) -> ScreenId {
    match self {
      Self::RUN => ScreenId::RUNCONFIG,
      Self::MODELS => ScreenId::MODELS,
      Self::SESSIONS => ScreenId::SESSIONS,
    }
  }
}

impl Screen for Home {
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
        MoveDirection::Up => {
          self.selected_action = self.selected_action.saturating_sub(1);
        },
        MoveDirection::Down => {
          self.selected_action =
            self.selected_action.saturating_add(1).min(HomeAction::COUNT - 1);
        },
        _ => {},
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          let screen_id =
            HomeAction::iter().nth(self.selected_action).map(|s| s.to_screen_id());
          if let Some(screen_id) = screen_id {
            command_tx.send(Action::Navigate(screen_id))?;
          }
        }
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let layout = Layout::default()
      .constraints(vec![
        Constraint::Percentage(10),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Percentage(10),
      ])
      .split(area);

    for (index, action) in HomeAction::iter().enumerate() {
      let inner_area = Layout::default()
        .constraints(vec![Constraint::Min(0), Constraint::Length(1)])
        .split(layout[index + 1]);
      let inner_area = inner_area[0];
      let button_width = 20;
      let padding = (inner_area.width - button_width) / 2;
      let inner_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
          Constraint::Length(padding),
          Constraint::Length(button_width),
          Constraint::Min(padding),
        ])
        .split(inner_area);
      let is_selected = index == self.selected_action;
      let button = Paragraph::new(action.to_string())
        .alignment(Alignment::Center)
        .block(stylized_button(is_selected));
      f.render_widget(button, inner_layout[1]);
    }

    Ok(())
  }
}
