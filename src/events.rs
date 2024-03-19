use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::{
  assets::{Candle, MarketEvent, MarketEventDetail},
  database::Database,
  portfolio::{
    balance::Balance,
    position::{Position, PositionExit, PositionUpdate},
    OrderEvent,
  },
  strategy::Signal,
  trading::{execution::FillEvent, SignalForceExit},
};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Event {
  Market(MarketEvent),
  Balance(Balance),
  Signal(Signal),
  SignalForceExit(SignalForceExit),
  Order(OrderEvent),
  Fill(FillEvent),
  PositionNew(Position),
  PositionUpdate(PositionUpdate),
  PositionExit(PositionExit),
}

// Messages to downstream consumers.
pub trait MessageTransmitter<Message> {
  /// Attempts to send a message to an external message subscriber.
  fn send(&mut self, message: Message);

  /// Attempts to send many messages to an external message subscriber.
  fn send_many(&mut self, messages: Vec<Message>);
}

// Sending to an external sink.
#[derive(Debug, Clone)]
pub struct EventTx {
  // Flag to communicate if the external receiver has been dropped.
  receiver_dropped: bool,
  // Channel transmitter to send events to an external sink.
  event_tx: mpsc::UnboundedSender<Event>,
}

impl MessageTransmitter<Event> for EventTx {
  fn send(&mut self, message: Event) {
    if self.receiver_dropped {
      return;
    }

    log::info!("MSG {:?} {:?}", self.event_tx.is_closed(), message);

    if self.event_tx.send(message).is_err() {
      warn!(
        action = "setting receiver_dropped = true",
        why = "event receiver dropped",
        "cannot send Events"
      );
      self.receiver_dropped = true;
    }
  }

  fn send_many(&mut self, messages: Vec<Event>) {
    if self.receiver_dropped {
      return;
    }

    messages.into_iter().for_each(|message| {
      let _ = self.event_tx.send(message);
    })
  }
}

impl EventTx {
  pub fn new(event_tx: mpsc::UnboundedSender<Event>) -> Self {
    Self { receiver_dropped: false, event_tx }
  }
}

// pub async fn core_events_listener(
//   mut event_receiver: mpsc::UnboundedReceiver<Event>,
//   database: Arc<Mutex<Database>>,
//   is_live: bool,
// ) {
//   while let Some(event) = event_receiver.recv().await {
//     match event {
//       Event::Market(ev) => match ev.detail {
//         MarketEventDetail::Candle(candle) => {
//           if is_live {
//             let mut database = database.lock().await;
//             let candles: Vec<Candle> = vec![candle];
//             let insert = database.add_candles(ev.asset, candles).await;
//             match insert {
//               Ok(_) => info!("Inserted new candle."),
//               Err(e) => warn!("Error inserting candle: {:?}", e),
//             }
//           }
//         },
//         _ => (), //info!("{:?}", ev),
//       },
//       _ => (), // info!("{:?}", event),
//     }
//   }
// }
