pub mod error;
pub mod execution;

use self::{error::TraderError, execution::Execution};
use crate::{
  assets::{Feed, MarketEventDetail, MarketFeed, Pair},
  core::Command,
  events::{Event, EventTx, MessageTransmitter},
  portfolio::Portfolio,
  strategy::Strategy,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use strum::{Display, EnumString};
use tokio::{
  sync::{broadcast, mpsc, Mutex},
  time::sleep,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Clone, Eq, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct SignalForceExit {
  pub time: DateTime<Utc>,
  pub asset: Pair,
}
impl SignalForceExit {
  fn from(asset: Pair, time: Option<DateTime<Utc>>) -> Self {
    let time = if time.is_some() { time.unwrap() } else { Utc::now() };
    SignalForceExit { time, asset }
  }
}

pub struct Trader {
  core_id: Uuid,
  pub pair: Pair,
  command_reciever: mpsc::Receiver<Command>,
  event_transmitter: EventTx,
  event_rx: broadcast::Receiver<Event>,
  event_queue: VecDeque<Event>,
  portfolio: Arc<Mutex<Portfolio>>,
  strategy: Strategy,
  execution: Execution,
  trading_is_live: bool,
}

impl Trader {
  pub fn builder() -> TraderBuilder {
    TraderBuilder::new()
  }
  pub async fn run(&mut self) -> Result<(), TraderError> {
    let _ = tokio::time::sleep(Duration::from_micros(200)).await;

    'trader_loop: loop {
      while let Some(command) = self.receive_remote_command() {
        match command {
          Command::Terminate(_) => break 'trader_loop,
          Command::ExitPosition(asset) => {
            self
              .event_queue
              .push_back(Event::SignalForceExit(SignalForceExit::from(asset, None)));
          },
          _ => continue,
        }
      }
      match self.event_rx.try_recv() {
        Ok(event) => {
          self.event_queue.push_back(event);
        },
        Err(e) => {
          let err_msg = format!("Error on trader event feed: {:?}", e);
          match e {
            broadcast::error::TryRecvError::Empty => {
              continue;
            },
            broadcast::error::TryRecvError::Lagged(num_skipped) => {
              log::warn!("Trader skipped {} messages (lag).", num_skipped);
              continue;
            },
            broadcast::error::TryRecvError::Closed => {
              log::warn!("{}", err_msg);
              let positions =
                self.portfolio.lock().await.open_positions(self.core_id).await;
              match positions {
                Ok(positions) => {
                  if positions.len() > 0 {
                    let last_update = positions.last().unwrap().meta.update_time;
                    self.event_queue.push_back(Event::SignalForceExit(
                      SignalForceExit::from(self.pair.clone(), Some(last_update)),
                    ));
                  } else {
                    break;
                  }
                },
                Err(e) => {
                  error!("{:?}", e)
                },
              }
            },
          }
        },
      }
      while let Some(event) = self.event_queue.pop_front() {
        match event {
          Event::Market(market_event) => {
            if market_event.pair == self.pair {
              match self.strategy.generate_signal(&market_event).await {
                Ok(Some(signal)) => {
                  self.event_transmitter.send(Event::Signal(signal.clone()));
                  self.event_queue.push_back(Event::Signal(signal));
                },
                Ok(None) => { /* No signal = do nothing*/ },
                Err(e) => {
                  error!("Exiting on strategy error. {}", e);
                  return Err(TraderError::from(e));
                },
              }
            }
            if let Some(position_update) = self
              .portfolio
              .lock()
              .await
              .update_from_market(self.core_id, market_event)
              .await?
            {
              self.event_transmitter.send(Event::PositionUpdate(position_update));
            }
          },
          Event::Signal(signal) => {
            match self
              .portfolio
              .lock()
              .await
              .generate_order(self.core_id, &signal, self.trading_is_live)
              .await
            {
              Ok(order) => {
                if let Some(order) = order {
                  self.event_transmitter.send(Event::Order(order.clone()));
                  self.event_queue.push_back(Event::Order(order));
                }
              },
              Err(e) => warn!("{}", e),
            }
          },
          Event::SignalForceExit(signal_force_exit) => {
            match self
              .portfolio
              .lock()
              .await
              .generate_exit_order(self.core_id, signal_force_exit, self.trading_is_live)
              .await
            {
              Ok(order) => {
                if let Some(order) = order {
                  self.event_transmitter.send(Event::Order(order.clone()));
                  self.event_queue.push_back(Event::Order(order));
                }
              },
              Err(e) => warn!("{}", e),
            }
          },
          Event::Order(order) => {
            match self.execution.generate_fill(&order, self.trading_is_live).await {
              Ok(fill) => {
                self.event_transmitter.send(Event::Fill(fill.clone()));
                self.event_queue.push_back(Event::Fill(fill));
              },
              Err(e) => {
                log::error!("{:?}", e);
              },
            }
          },
          Event::Fill(fill) => {
            let fill_side_effect_events =
              self.portfolio.lock().await.update_from_fill(self.core_id, &fill).await?;
            self.event_transmitter.send_many(fill_side_effect_events);
          },
          _ => {},
        }
      }

      debug!(
        engine_id = &*self.core_id.to_string(),
        asset = &*format!("{:?}", self.pair),
        "Trader trading loop stopped"
      );
    }

    info!("Trader {} shutting down.", self.pair);
    Ok(())
  }
  fn receive_remote_command(&mut self) -> Option<Command> {
    match self.command_reciever.try_recv() {
      Ok(command) => {
        debug!(
          engine_id = &*self.core_id.to_string(),
          asset = &*format!("{:?}", self.pair),
          command = &*format!("{:?}", command),
          "Trader received remote command"
        );
        Some(command)
      },
      Err(err) => match err {
        mpsc::error::TryRecvError::Empty => None,
        mpsc::error::TryRecvError::Disconnected => {
          warn!(
            action = "synthesising a Command::Terminate",
            "remote Command transmitter has been dropped"
          );
          Some(Command::Terminate("remote command transmitter dropped".to_owned()))
        },
      },
    }
  }
}

pub struct TraderBuilder {
  core_id: Option<Uuid>,
  pair: Option<Pair>,
  market_feed: Option<MarketFeed>,
  command_reciever: Option<mpsc::Receiver<Command>>,
  event_transmitter: Option<EventTx>,
  event_rx: Option<broadcast::Receiver<Event>>,
  event_queue: Option<VecDeque<Event>>,
  portfolio: Option<Arc<Mutex<Portfolio>>>,
  strategy: Option<Strategy>,
  execution: Option<Execution>,
  trading_is_live: Option<bool>,
}
impl TraderBuilder {
  pub fn new() -> TraderBuilder {
    TraderBuilder {
      core_id: None,
      command_reciever: None,
      pair: None,
      trading_is_live: None,
      event_transmitter: None,
      event_rx: None,
      portfolio: None,
      market_feed: None,
      event_queue: None,
      execution: None,
      strategy: None,
    }
  }
  pub fn core_id(self, value: Uuid) -> Self {
    Self { core_id: Some(value), ..self }
  }

  pub fn pair(self, value: Pair) -> Self {
    Self { pair: Some(value), ..self }
  }

  pub fn command_reciever(self, value: mpsc::Receiver<Command>) -> Self {
    Self { command_reciever: Some(value), ..self }
  }

  pub fn event_transmitter(self, value: EventTx) -> Self {
    Self { event_transmitter: Some(value), ..self }
  }

  pub fn portfolio(self, value: Arc<Mutex<Portfolio>>) -> Self {
    Self { portfolio: Some(value), ..self }
  }

  pub fn market_feed(self, value: MarketFeed) -> Self {
    Self { market_feed: Some(value), ..self }
  }

  pub fn strategy(self, value: Strategy) -> Self {
    Self { strategy: Some(value), ..self }
  }

  pub fn execution(self, value: Execution) -> Self {
    Self { execution: Some(value), ..self }
  }

  pub fn trading_is_live(self, value: bool) -> Self {
    Self { trading_is_live: Some(value), ..self }
  }

  pub fn event_rx(self, value: broadcast::Receiver<Event>) -> Self {
    Self { event_rx: Some(value), ..self }
  }

  pub fn build(self) -> Result<Trader, TraderError> {
    Ok(Trader {
      core_id: self.core_id.ok_or(TraderError::BuilderIncomplete("engine_id"))?,
      pair: self.pair.ok_or(TraderError::BuilderIncomplete("pair"))?,
      command_reciever: self
        .command_reciever
        .ok_or(TraderError::BuilderIncomplete("command_rx"))?,
      event_transmitter: self
        .event_transmitter
        .ok_or(TraderError::BuilderIncomplete("event_tx"))?,
      event_rx: self.event_rx.ok_or(TraderError::BuilderIncomplete("event_rx"))?,
      event_queue: VecDeque::with_capacity(20),
      portfolio: self.portfolio.ok_or(TraderError::BuilderIncomplete("portfolio"))?,
      strategy: self.strategy.ok_or(TraderError::BuilderIncomplete("strategy"))?,
      execution: self.execution.ok_or(TraderError::BuilderIncomplete("execution"))?,
      trading_is_live: self
        .trading_is_live
        .ok_or(TraderError::BuilderIncomplete("trading_is_live"))?,
    })
  }
}
