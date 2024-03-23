use super::{Screen, ScreenId};
use crate::{
  action::{Action, MoveDirection},
  assets::Pair,
  components::{
    form::{input::Input, select::Select},
    style::{
      button, button_style, centered_text, default_action_block_style, default_header,
      default_layout, outer_container_block, stylized_block,
    },
    ListDisplay,
  },
  config::{Config, KeyBindings},
  core::Command,
  strategy::{get_generated_models, ModelId},
};
use chrono::{DateTime, Duration, Utc};
use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

const MODEL_SYNC_DURATION: Duration = Duration::milliseconds(500);

#[derive(Default, Serialize, Clone, PartialEq, Debug)]
pub struct CoreConfiguration {
  pub run_live: bool,
  pub n_days_to_fetch: u64,
  pub starting_equity: f64,
  pub backtest_last_n_candles: usize,
  pub exchange_fee: f64,
  pub pair: Pair,
  pub model_name: String,
}

#[derive(Default, PartialEq, EnumIter, EnumCount, Clone)]
enum SelectedField {
  #[default]
  Pair,
  Model,
  StartingEquity,
  ExchangeFee,
  BacktestLastNCandles,
  FetchLastNDays,
  Actions,
}

#[derive(Default)]
pub struct RunConfig {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  selected_field: SelectedField,
  selected_field_index: usize,
  is_field_being_edited: bool,
  selected_action: usize,
  fetch_last_n_days: Input,
  backtest_last_n_candles: Input,
  starting_equity: Input,
  exchange_fee: Input,
  model_id: Select<ModelId>,
  pair: Select<Pair>,
  last_model_sync: DateTime<Utc>,
}

impl RunConfig {
  pub fn new() -> Self {
    let mut config = Self {
      fetch_last_n_days: Input::new(Some(0.0), Some("Fetch N days history".to_string())),
      backtest_last_n_candles: Input::new(
        Some(1440.0),
        Some("(Backtest) N Candles".to_string()),
      ),
      starting_equity: Input::new(Some(1000.0), Some("Starting equity".to_string())),
      exchange_fee: Input::new(Some(0.0), Some("Exchange fee".to_string())),
      pair: Select::new(
        vec![Pair::BTCUSDT, Pair::ETHBTC],
        Some(Pair::BTCUSDT),
        Some("Pair".to_string()),
      ),
      model_id: Select::new(vec![], None, Some("Model".to_string())),
      selected_field_index: 0,
      selected_field: SelectedField::Pair,
      last_model_sync: Utc::now(),
      ..Self::default()
    };
    config.set_field_active(SelectedField::Pair);
    config
  }

  fn activate_field(&mut self, selected_field: SelectedField) {}

  fn set_field_active(&mut self, selected_field: SelectedField) {
    self.model_id.set_active(selected_field == SelectedField::Model);
    self.pair.set_active(selected_field == SelectedField::Pair);
    self.fetch_last_n_days.set_active(selected_field == SelectedField::FetchLastNDays);
    self
      .backtest_last_n_candles
      .set_active(selected_field == SelectedField::BacktestLastNCandles);
    self.starting_equity.set_active(selected_field == SelectedField::StartingEquity);
    self.exchange_fee.set_active(selected_field == SelectedField::ExchangeFee);
  }

  fn sync_models(&mut self) -> Result<()> {
    if self.last_model_sync + MODEL_SYNC_DURATION < Utc::now() {
      let metadata_list = get_generated_models()?;
      let model_id_list: Vec<ModelId> =
        metadata_list.iter().map(|metadata| metadata.to_model_id()).collect();
      let sorted_list = self.model_id.set_options(model_id_list);
      self.last_model_sync = Utc::now();
    }
    Ok(())
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
      Action::Tick => {
        self.sync_models()?;
      },
      Action::Move(direction) => match direction {
        MoveDirection::Left => {
          if self.selected_field == SelectedField::Actions {
            self.selected_action = self.selected_action.saturating_sub(1);
          }
        },
        MoveDirection::Right => {
          if self.selected_field == SelectedField::Actions {
            self.selected_action = self.selected_action.saturating_add(1).min(2);
          }
        },
        MoveDirection::Down => {
          if self.is_field_being_edited {
            match self.selected_field {
              SelectedField::Pair => self.pair.edit_next(),
              SelectedField::Model => self.model_id.edit_next(),
              _ => (),
            };
          } else {
            self.selected_field_index =
              (self.selected_field_index + 1) % SelectedField::COUNT;
            self.selected_field = SelectedField::iter()
              .nth(self.selected_field_index)
              .unwrap_or(SelectedField::Actions);
            self.set_field_active(self.selected_field.clone());
          }
        },
        MoveDirection::Up => {
          if self.is_field_being_edited {
            match self.selected_field {
              SelectedField::Pair => self.pair.edit_previous(),
              SelectedField::Model => self.model_id.edit_previous(),
              _ => (),
            };
          } else {
            self.selected_field_index = self.selected_field_index.saturating_sub(1);
            self.selected_field = SelectedField::iter()
              .nth(self.selected_field_index)
              .unwrap_or(SelectedField::Actions);
            self.set_field_active(self.selected_field.clone());
          }
        },
      },
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          if self.selected_field == SelectedField::Actions {
            let options = self.pair.value().zip(self.model_id.value());
            let screen_id = if self.selected_action == 2 {
              command_tx.send(Action::Navigate(ScreenId::HOME))?;
            } else if let Some((pair, model_id)) = options {
              command_tx.send(Action::CoreCommand(Command::Start(
                CoreConfiguration {
                  run_live: self.selected_action == 1,
                  n_days_to_fetch: self.fetch_last_n_days.value() as u64,
                  starting_equity: self.starting_equity.value(),
                  backtest_last_n_candles: self.backtest_last_n_candles.value() as usize,
                  exchange_fee: self.exchange_fee.value(),
                  model_name: model_id.name.clone(),
                  pair,
                },
              )))?;
            };
          } else {
            // ACTIVATE INPUTS
            let is_field_being_edited = match self.selected_field {
              SelectedField::Pair => self.pair.toggle_edit(),
              SelectedField::Model => self.model_id.toggle_edit(),
              SelectedField::ExchangeFee => self.exchange_fee.toggle_edit(),
              SelectedField::StartingEquity => self.starting_equity.toggle_edit(),
              SelectedField::FetchLastNDays => self.fetch_last_n_days.toggle_edit(),
              SelectedField::BacktestLastNCandles => {
                self.backtest_last_n_candles.toggle_edit()
              },
              SelectedField::Actions => false,
            };
            self.is_field_being_edited = is_field_being_edited
          }
        }
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    f.render_widget(outer_container_block(), area);
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Min(0), Constraint::Length(3)])
      .split(area);

    let form_layout = Layout::default()
      .constraints(vec![
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(0),
      ])
      .split(content_layout[0]);

    // Pair
    self.pair.draw(f, form_layout[0])?;

    // Model
    self.model_id.draw(f, form_layout[1])?;

    // Starting Equity
    self.starting_equity.draw(f, form_layout[2])?;

    // Exchange Fee
    self.exchange_fee.draw(f, form_layout[3])?;

    // Backtest Last N Candles
    self.backtest_last_n_candles.draw(f, form_layout[4])?;

    // Last N days fetch
    self.fetch_last_n_days.draw(f, form_layout[5])?;

    let button_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Percentage(9),
        Constraint::Percentage(26),
        Constraint::Length(1),
        Constraint::Percentage(26),
        Constraint::Length(1),
        Constraint::Percentage(26),
        Constraint::Percentage(9),
      ])
      .split(content_layout[1]);

    match self.selected_field {
      SelectedField::Pair => self.pair.draw_edit(f, content_layout[0])?,
      SelectedField::Model => self.model_id.draw_edit(f, content_layout[0])?,
      SelectedField::StartingEquity => {
        self.starting_equity.draw_edit(f, content_layout[0])?
      },
      SelectedField::ExchangeFee => self.exchange_fee.draw_edit(f, content_layout[0])?,
      SelectedField::BacktestLastNCandles => {
        self.backtest_last_n_candles.draw_edit(f, content_layout[0])?
      },
      SelectedField::FetchLastNDays => {
        self.fetch_last_n_days.draw_edit(f, content_layout[0])?
      },
      SelectedField::Actions => (),
    };

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
    f.render_widget(
      button(
        "BACK",
        self.selected_field == SelectedField::Actions && self.selected_action == 2,
      ),
      button_layout[5],
    );

    Ok(())
  }
}
