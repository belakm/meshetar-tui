pub mod error;

use self::error::StrategyError;
use crate::{
  assets::{Candle, MarketEvent, MarketEventDetail, MarketMeta, Pair},
  components::{
    style::{default_style, DEFAULT_THEME},
    ListDisplay,
  },
  utils::{
    formatting::{generate_petname, time_ago, timestamp_to_dt},
    remove_vec_items_from_start,
  },
};
use chrono::{DateTime, Utc};
use color_eyre::owo_colors::OwoColorize;
use futures::TryFutureExt;
use petname::Petnames;
use pyo3::{prelude::*, types::PyModule};
use ratatui::{
  prelude::{Constraint, Direction, Layout},
  style::Style,
  widgets::{Block, Paragraph},
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashMap, path::Path};
use tokio::fs;
use uuid::Uuid;

#[derive(Default, Clone)]
pub struct ModelId {
  pub name: String,
  pub uuid: Uuid,
  pub pair: Pair,
}
impl std::fmt::Display for ModelId {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{}-{}", self.name.clone(), self.pair.to_string())
  }
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Signal {
  pub time: DateTime<Utc>,
  pub pair: Pair,
  pub market_meta: MarketMeta,
  pub signals: HashMap<Decision, SignalStrength>,
}

impl PartialOrd for Signal {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    // First, compare by the `time` field
    match self.time.cmp(&other.time) {
      Ordering::Equal => {
        // If times are equal, compare by the `asset` field
        match self.pair.partial_cmp(&other.pair) {
          Some(Ordering::Equal) => {
            // If assets are equal, compare by the `market_meta` field
            self.market_meta.partial_cmp(&other.market_meta)
          },
          other => other,
        }
      },
      other => Some(other),
    }
  }
}

#[derive(
  Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize,
)]
pub enum Decision {
  Long,
  CloseLong,
  Short,
  CloseShort,
}

impl Default for Decision {
  fn default() -> Self {
    Self::Long
  }
}

impl Decision {
  pub fn is_long(&self) -> bool {
    matches!(self, Decision::Long)
  }
  pub fn is_short(&self) -> bool {
    matches!(self, Decision::Short)
  }
  pub fn is_entry(&self) -> bool {
    matches!(self, Decision::Short | Decision::Long)
  }
  pub fn is_exit(&self) -> bool {
    matches!(self, Decision::CloseLong | Decision::CloseShort)
  }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct SignalStrength(pub f64);

pub struct Strategy {
  pair: Pair,
  model_name: String,
}
impl Strategy {
  pub fn new(pair: Pair, model_name: String) -> Self {
    Strategy { pair, model_name }
  }
  pub async fn generate_signal(
    &mut self,
    market_event: &MarketEvent,
  ) -> Result<Option<Signal>, StrategyError> {
    if let MarketEventDetail::BacktestCandle((_, signal)) = &market_event.detail {
      Ok(signal.to_owned())
    } else if let MarketEventDetail::Candle(candle) = &market_event.detail {
      // Run model
      let pyscript = include_str!("../../models/run_model.py");
      let args =
        (candle.open_time.to_rfc3339(), self.pair.to_string(), self.model_name.clone());
      let model_output = "hold".to_string(); // run_candle(pyscript, args)?;
      let signals = generate_signals_map(&model_output);
      if signals.len() == 0 {
        return Ok(None);
      }
      let time = Utc::now();
      let signal = Signal {
        time,
        pair: self.pair.clone(),
        market_meta: MarketMeta { close: candle.close, time },
        signals,
      };
      Ok(Some(signal))
    } else {
      Ok(None)
    }
  }

  /// buffer_n_of_candles - number of candles that are required for analysis of the "first" candle
  pub async fn generate_backtest_signals(
    open_time: DateTime<Utc>,
    candles: Vec<Candle>,
    buffer_n_of_candles: usize,
    pair: Pair,
    model_name: String,
  ) -> Result<Option<Vec<Option<Signal>>>, StrategyError> {
    let pyscript = include_str!("../../models/backtest.py");
    let args = (open_time.to_rfc3339(), pair.to_string(), model_name);
    let model_output = run_backtest(pyscript, args)?;
    let candles_that_were_analyzed = remove_vec_items_from_start(candles, 0);
    let mut candles_with_signals: Vec<(Candle, HashMap<Decision, SignalStrength>)> =
      Vec::new();
    for candle in candles_that_were_analyzed {
      let raw_signal =
        model_output.iter().find(|(_, datetime)| datetime == &candle.open_time);
      let signal_map = match raw_signal {
        Some(raw_signal) => generate_signals_map(&raw_signal.0),
        None => generate_signals_map("hold"),
      };
      candles_with_signals.push((candle, signal_map));
    }
    let signals: Vec<Option<Signal>> = candles_with_signals
      .iter()
      .map(|(candle, signal_map)| {
        if signal_map.len() == 0 {
          None
        } else {
          Some(Signal {
            time: candle.close_time,
            pair: pair.clone(),
            market_meta: MarketMeta { close: candle.close, time: candle.close_time },
            signals: signal_map.to_owned(),
          })
        }
      })
      .collect();

    Ok(Some(signals))
  }
}

fn generate_signals_map(model_output: &str) -> HashMap<Decision, SignalStrength> {
  let mut signals = HashMap::with_capacity(4);
  match model_output {
    "sell" => {
      // signals.insert(Decision::Short, SignalStrength(1.0));
      signals.insert(Decision::CloseLong, SignalStrength(1.0));
    },
    "buy" => {
      signals.insert(Decision::Long, SignalStrength(1.0));
      // signals.insert(Decision::CloseShort, SignalStrength(1.0));
    },
    _ => (),
  };
  signals
}

fn run_candle(script: &str, args: (String, String, String)) -> PyResult<String> {
  let result: PyResult<String> = Python::with_gil(|py| {
    let activators = PyModule::from_code(py, script, "activators.py", "activators")?;
    let prediction: String = activators.getattr("run")?.call1(args)?.extract()?;
    Ok(prediction)
  });
  Ok(result?)
}

fn run_backtest(
  script: &str,
  args: (String, String, String),
) -> PyResult<Vec<(String, DateTime<Utc>)>> {
  let result: PyResult<Vec<_>> = Python::with_gil(|py| {
    let activators = PyModule::from_code(py, script, "activators.py", "activators")?;
    let signals: Vec<(String, String)> =
      activators.getattr("backtest")?.call1(args)?.extract()?;
    let mut parsed_signals: Vec<(String, DateTime<Utc>)> = Vec::new();
    for (time, signal) in signals {
      let datetime = DateTime::parse_from_rfc3339(&time).unwrap().with_timezone(&Utc);
      parsed_signals.push((signal, datetime));
    }
    Ok(parsed_signals)
  });
  Ok(result?)
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ModelMetadata {
  pub created_at: DateTime<Utc>,
  pair: Pair,
  is_finished: bool,
  error: String,
  name: String,
  uuid: Uuid,
}

impl ModelMetadata {
  pub fn new(
    created_at: DateTime<Utc>,
    pair: Pair,
    is_finished: bool,
    error: String,
  ) -> Self {
    Self {
      created_at,
      pair,
      is_finished,
      error,
      name: generate_petname(),
      uuid: Uuid::new_v4(),
    }
  }

  pub fn to_model_id(&self) -> ModelId {
    ModelId { name: self.name.clone(), uuid: self.uuid, pair: self.pair.clone() }
  }
}

impl ListDisplay for ModelMetadata {
  fn draw(
    &mut self,
    f: &mut ratatui::Frame<'_>,
    area: ratatui::prelude::Rect,
    active: bool,
  ) -> color_eyre::eyre::Result<()> {
    f.render_widget(Block::default().style(default_style(active)), area.clone());
    let row_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Max(8),
        Constraint::Length(8),
        Constraint::Min(0),
        Constraint::Length(20),
        Constraint::Length(8),
      ])
      .split(area);

    let status = match self.is_finished {
      true => {
        if self.error.len() == 0 {
          "🟢 OK"
        } else {
          "🟪 ERR"
        }
      },
      false => "🔵 WORK",
    };

    let has_error = self.error != "";
    let msg = if !self.is_finished {
      "Generating".to_string()
    } else if has_error {
      self.error.clone()
    } else {
      "Ready".to_string()
    };
    let error_style = if has_error {
      default_style(active).fg(DEFAULT_THEME.text_critical)
    } else {
      default_style(active).fg(DEFAULT_THEME.text_dimmed)
    };

    f.render_widget(Paragraph::new(status), row_layout[0]);
    f.render_widget(Paragraph::new(self.pair.to_string()), row_layout[1]);
    f.render_widget(Paragraph::new(msg).style(error_style), row_layout[2]);
    f.render_widget(Paragraph::new(self.name.clone()), row_layout[3]);
    f.render_widget(Paragraph::new(time_ago(self.created_at)), row_layout[4]);

    Ok(())
  }
  fn draw_header(
    &mut self,
    f: &mut ratatui::Frame<'_>,
    area: ratatui::prelude::Rect,
  ) -> color_eyre::eyre::Result<()> {
    f.render_widget(Block::default().style(default_style(false)), area.clone());
    let header_style = Style::default().fg(DEFAULT_THEME.text_dimmed);
    let row_layout = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Max(8),
        Constraint::Length(8),
        Constraint::Min(0),
        Constraint::Length(20),
        Constraint::Length(8),
      ])
      .split(area);
    f.render_widget(Paragraph::new(""), row_layout[0]);
    f.render_widget(Paragraph::new("Pair").style(header_style), row_layout[1]);
    f.render_widget(Paragraph::new("Status").style(header_style), row_layout[2]);
    f.render_widget(Paragraph::new("Pet name").style(header_style), row_layout[3]);
    f.render_widget(Paragraph::new("Created").style(header_style), row_layout[4]);
    Ok(())
  }
}

pub async fn generate_new_model(pair: Pair) -> Result<(), StrategyError> {
  let created_at = Utc::now();
  let model_metadata =
    ModelMetadata::new(created_at.clone(), pair.clone(), false, "".to_string());
  let file_name = model_metadata.name.clone();
  let file_path = format!("models/generated/{}", file_name.clone());
  match fs::create_dir(file_path.clone()).await {
    Ok(_) => {
      fs::File::create(format!("{file_path}/meta.toml"))
        .await
        .map_err(|e| StrategyError::FileError(e.to_string()))?;
      fs::write(
        format!("{file_path}/meta.toml"),
        toml::to_string_pretty::<ModelMetadata>(&model_metadata)
          .map_err(|e| StrategyError::FileError(e.to_string()))?,
      )
      .map_err(|e| StrategyError::FileError(e.to_string()))
      .await?;
      let result: PyResult<()> = Python::with_gil(|py| {
        let pyscript = include_str!("../../models/create_model.py");
        let args = (pair.to_string(), file_name);
        let activators =
          PyModule::from_code(py, pyscript, "activators.py", "activators")?;
        activators.getattr("new_model")?.call1(args)?;
        Ok(())
      });
      match result {
        Ok(_) => {
          fs::write(
            format!("{file_path}/meta.toml"),
            toml::to_string_pretty::<ModelMetadata>(&model_metadata)
              .map_err(|e| StrategyError::FileError(e.to_string()))?,
          )
          .map_err(|e| StrategyError::FileError(e.to_string()))
          .await?;
          Ok(())
        },
        Err(e) => {
          fs::write(
            format!("{file_path}/meta.toml"),
            toml::to_string_pretty::<ModelMetadata>(&model_metadata)
              .map_err(|e| StrategyError::FileError(e.to_string()))?,
          )
          .map_err(|e| StrategyError::FileError(e.to_string()))
          .await?;
          Err(StrategyError::from(e))
        },
      }
    },
    Err(e) => Err(StrategyError::FileError(format!(
      "Error on path: {:?} - {}",
      file_path,
      e.to_string()
    ))),
  }
}

pub fn get_generated_models() -> color_eyre::Result<Vec<ModelMetadata>> {
  let path = Path::new("models/generated");
  let mut metadata_list: Vec<ModelMetadata> = Vec::new();
  for entry in std::fs::read_dir(path)? {
    let entry = entry?;
    if entry.path().is_dir() {
      let config_path = entry.path().join("meta.toml");
      if config_path.exists() && config_path.is_file() {
        let file = std::fs::read_to_string(&config_path)?;
        match parse_model_metadata(&file) {
          Ok(metadata) => {
            metadata_list.push(metadata);
          },
          Err(e) => log::warn!("Error on reading modal metafile: {:?}", e),
        }
      }
    }
  }
  metadata_list.sort_by_cached_key(|item| item.created_at);
  metadata_list.reverse();
  Ok(metadata_list)
}

pub fn parse_model_metadata(contents: &str) -> color_eyre::Result<ModelMetadata> {
  let value = contents.parse::<toml::Value>()?;
  let created_at: DateTime<Utc> = value
    .get("created_at")
    .and_then(toml::Value::as_str)
    .unwrap_or_default()
    .to_string()
    .parse()?;
  let error: String =
    value.get("error").and_then(toml::Value::as_str).unwrap_or_default().to_string();
  let pair: Pair =
    value.get("pair").and_then(toml::Value::as_str).unwrap_or_default().parse()?;
  let uuid: Uuid =
    value.get("uuid").and_then(toml::Value::as_str).unwrap_or_default().parse()?;
  let name: String =
    value.get("name").and_then(toml::Value::as_str).unwrap_or_default().parse()?;
  let is_finished: bool =
    value.get("is_finished").and_then(toml::Value::as_bool).unwrap_or_default();
  Ok(ModelMetadata { created_at, pair, is_finished, error, name, uuid })
}
