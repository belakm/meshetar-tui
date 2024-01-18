use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
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

#[derive(Default, Serialize, Clone, PartialEq, Debug)]
pub struct CoreConfiguration {
  pub run_live: bool,
  pub n_days_to_fetch: u64,
  pub starting_equity: f64,
  pub backtest_last_n_candles: usize,
  pub exchange_fee: f64,
}

#[derive(Default, PartialEq, EnumIter, EnumCount, Clone)]
enum SelectedField {
  StartingEquity,
  ExchangeFee,
  BacktestLastNCandles,
  FetchLastNDays,
  #[default]
  Actions,
}

#[derive(Default)]
pub struct RunConfig {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_field: SelectedField,
  selected_field_index: usize,
  selected_action: usize,
  fetch_last_n_days: Input,
  backtest_last_n_candles: Input,
  starting_equity: Input,
  exchange_fee: Input,
}

impl RunConfig {
  pub fn new() -> Self {
    Self {
      fetch_last_n_days: Input::new(
        Some(0.0),
        Some("How many days of history to fetch".to_string()),
      ),
      backtest_last_n_candles: Input::new(
        Some(1440.0),
        Some("(Backtest) N Candles".to_string()),
      ),
      starting_equity: Input::new(Some(1000.0), Some("Starting equity".to_string())),
      exchange_fee: Input::new(Some(0.0), Some("Exchange fee".to_string())),
      selected_field_index: 4,
      ..Self::default()
    }
  }

  fn set_field_active(&mut self, selected_field: SelectedField) {
    self.fetch_last_n_days.set_active(selected_field == SelectedField::FetchLastNDays);
    self
      .backtest_last_n_candles
      .set_active(selected_field == SelectedField::BacktestLastNCandles);
    self.starting_equity.set_active(selected_field == SelectedField::StartingEquity);
    self.exchange_fee.set_active(selected_field == SelectedField::ExchangeFee);
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
            let screen_id = if self.selected_action == 0 {
              ScreenId::BACKTEST
            } else {
              ScreenId::RUNNING
            };
            command_tx.send(Action::CoreCommand(Command::Start(CoreConfiguration {
              run_live: self.selected_action == 1,
              n_days_to_fetch: self.fetch_last_n_days.value() as u64,
              starting_equity: self.starting_equity.value(),
              backtest_last_n_candles: self.backtest_last_n_candles.value() as usize,
              exchange_fee: self.exchange_fee.value(),
            })))?;
            command_tx.send(Action::Navigate(screen_id))?;
          } else {
            // ACTIVATE INPUTS
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
      .constraints(vec![
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Min(0),
      ])
      .split(content_layout[0]);

    //
    // Maybe later show some extra detail like how much days we have in database
    //

    // Default Pair
    f.render_widget(
      Paragraph::new("USDT / BTC (fixed)")
        .block(Block::new().style(default_action_block_style(false, false))),
      form_layout[0],
    );

    // Starting Equity
    self.starting_equity.draw(f, form_layout[1])?;

    // Exchange Fee
    self.exchange_fee.draw(f, form_layout[2])?;

    // Backtest Last N Candles
    self.backtest_last_n_candles.draw(f, form_layout[3])?;

    // Last N days fetch
    self.fetch_last_n_days.draw(f, form_layout[4])?;

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
        "BACKTEST",
        self.selected_field == SelectedField::Actions && self.selected_action == 0,
      ),
      button_layout[1],
    );
    f.render_widget(
      button(
        "RUN",
        self.selected_field == SelectedField::Actions && self.selected_action == 1,
      ),
      button_layout[3],
    );

    Ok(())
  }
}
