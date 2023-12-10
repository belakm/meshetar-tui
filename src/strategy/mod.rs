pub mod error;
// pub mod routes;

use self::error::StrategyError;
use crate::{
    assets::{Asset, Candle, MarketEvent, MarketEventDetail, MarketMeta},
    utils::remove_vec_items_from_start,
};
use chrono::{DateTime, Utc};
use pyo3::{prelude::*, types::PyModule};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashMap};

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Signal {
    pub time: DateTime<Utc>,
    pub asset: Asset,
    pub market_meta: MarketMeta,
    pub signals: HashMap<Decision, SignalStrength>,
}

impl PartialOrd for Signal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // First, compare by the `time` field
        match self.time.cmp(&other.time) {
            Ordering::Equal => {
                // If times are equal, compare by the `asset` field
                match self.asset.partial_cmp(&other.asset) {
                    Some(Ordering::Equal) => {
                        // If assets are equal, compare by the `market_meta` field
                        self.market_meta.partial_cmp(&other.market_meta)
                    }
                    other => other,
                }
            }
            other => Some(other),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
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
    asset: Asset,
}
impl Strategy {
    pub fn new(asset: Asset) -> Self {
        Strategy { asset }
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
            let args = (candle.open_time.to_rfc3339(),);
            let model_output = run_candle(pyscript, args)?;
            let signals = generate_signals_map(&model_output);
            if signals.len() == 0 {
                return Ok(None);
            }
            let time = Utc::now();
            let signal = Signal {
                time,
                asset: self.asset.clone(),
                market_meta: MarketMeta {
                    close: candle.close,
                    time,
                },
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
        asset: Asset,
        buffer_n_of_candles: usize,
    ) -> Result<Option<Vec<Option<Signal>>>, StrategyError> {
        let pyscript = include_str!("../../models/backtest.py");
        let args = (open_time.to_rfc3339(), asset.to_string());
        let model_output = run_backtest(pyscript, args)?;
        let candles_that_were_analyzed = remove_vec_items_from_start(candles, 0);

        info!("FIRST CANDLE {:?}", candles_that_were_analyzed.first());

        let mut candles_with_signals: Vec<(Candle, HashMap<Decision, SignalStrength>)> = Vec::new();
        for candle in candles_that_were_analyzed {
            let raw_signal = model_output
                .iter()
                .find(|(_, datetime)| datetime == &candle.open_time);
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
                        asset: asset.clone(),
                        market_meta: MarketMeta {
                            close: candle.close,
                            time: candle.close_time,
                        },
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
        }
        "buy" => {
            signals.insert(Decision::Long, SignalStrength(1.0));
            // signals.insert(Decision::CloseShort, SignalStrength(1.0));
        }
        _ => (),
    };
    signals
}

fn run_candle(script: &str, args: (String,)) -> PyResult<String> {
    let result: PyResult<String> = Python::with_gil(|py| {
        let activators = PyModule::from_code(py, script, "activators.py", "activators")?;
        let prediction: String = activators.getattr("run")?.call1(args)?.extract()?;
        Ok(prediction)
    });
    Ok(result?)
}

fn run_backtest(script: &str, args: (String, String)) -> PyResult<Vec<(String, DateTime<Utc>)>> {
    let result: PyResult<Vec<_>> = Python::with_gil(|py| {
        let activators = PyModule::from_code(py, script, "activators.py", "activators")?;
        let signals: Vec<(String, String)> =
            activators.getattr("backtest")?.call1(args)?.extract()?;
        let mut parsed_signals: Vec<(String, DateTime<Utc>)> = Vec::new();
        for (time, signal) in signals {
            let datetime = DateTime::parse_from_rfc3339(&time)
                .unwrap()
                .with_timezone(&Utc);
            parsed_signals.push((signal, datetime));
        }
        Ok(parsed_signals)
    });
    Ok(result?)
}
