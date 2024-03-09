use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
  assets::Pair,
  components::{
    form::input::Input,
    style::{
      button, button_style, centered_text, default_action_block_style, default_header,
      default_layout, logo, outer_container_block, stylized_block,
    },
  },
  config::{Config, KeyBindings},
  core::Command,
};
use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default, PartialEq, EnumIter, EnumCount, Clone)]
enum SelectedField {
  #[default]
  Actions,
}

#[derive(Default)]
pub struct ModelConfig {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_field: SelectedField,
  selected_field_index: usize,
  selected_action: usize,
  selected_pair: Pair,
}

impl ModelConfig {
  pub fn new() -> Self {
    Self { selected_field_index: 0, selected_pair: Pair::BTCUSDT, ..Self::default() }
  }

  fn set_field_active(&mut self, selected_field: SelectedField) {}
}

impl Screen for ModelConfig {
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
          if self.selected_field == SelectedField::Actions {
            self.selected_action = self.selected_action.saturating_sub(1);
          }
        },
        MoveDirection::Right => {
          if self.selected_field == SelectedField::Actions {
            self.selected_action = 1.min(self.selected_action + 1);
          }
        },
        MoveDirection::Down => {
          self.selected_field_index =
            (self.selected_field_index + 1) % SelectedField::COUNT;
          self.selected_field = SelectedField::iter()
            .nth(self.selected_field_index)
            .unwrap_or(SelectedField::Actions);
          self.set_field_active(self.selected_field.clone());
        },
        MoveDirection::Up => {
          self.selected_field_index = self.selected_field_index.saturating_sub(1);
          self.selected_field = SelectedField::iter()
            .nth(self.selected_field_index)
            .unwrap_or(SelectedField::Actions);
          self.set_field_active(self.selected_field.clone());
        },
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          if self.selected_field == SelectedField::Actions {
            if self.selected_action == 0 {
              command_tx.send(Action::GenerateModel(self.selected_pair))?;
            }
            command_tx.send(Action::Navigate(ScreenId::MODELS))?;
          }
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
    let form_layout = Layout::default()
      .constraints(vec![Constraint::Length(4), Constraint::Min(0)])
      .split(content_layout[0]);

    //
    // Maybe later show some extra detail like how much days we have in database
    //

    // Default Pair
    f.render_widget(
      Paragraph::new("BTC / USDT (fixed)")
        .block(Block::new().style(default_action_block_style(false, false))),
      form_layout[0],
    );

    // Starting Equity
    // self.starting_equity.draw(f, form_layout[1])?;

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

    f.render_widget(
      button(
        "Generate",
        self.selected_field == SelectedField::Actions && self.selected_action == 0,
      ),
      button_layout[1],
    );
    f.render_widget(
      button(
        "Back",
        self.selected_field == SelectedField::Actions && self.selected_action == 1,
      ),
      button_layout[3],
    );

    Ok(())
  }
}
