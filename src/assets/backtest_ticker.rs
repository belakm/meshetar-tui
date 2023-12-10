use super::{error::AssetError, Asset, MarketEvent, MarketEventDetail};
use crate::{
    database::Database,
    strategy::{Signal, Strategy},
    utils::remove_vec_items_from_start,
};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{self, UnboundedReceiver},
    Mutex,
};
use tracing::info;

pub async fn new_ticker(
    asset: Asset,
    database: Arc<Mutex<Database>>,
    last_n_candles: usize,
    buffer_n_of_candles: usize,
) -> Result<UnboundedReceiver<MarketEvent>, AssetError> {
    let (tx, rx) = mpsc::unbounded_channel();
    let candles = database
        .lock()
        .await
        .fetch_all_candles(asset.clone())
        .await?;

    info!("debug {}{}", candles.len(), last_n_candles);

    let skip_n_candles = candles.len() - last_n_candles;

    // take only specified number of candles
    let candles = remove_vec_items_from_start(candles, skip_n_candles);

    tokio::spawn(async move {
        let candles_copy = candles.clone();
        let open_time = candles_copy
            .first()
            .expect("No candles for backtesting :<")
            .open_time;
        match Strategy::generate_backtest_signals(
            open_time,
            candles_copy,
            asset.clone(),
            buffer_n_of_candles,
        )
        .await
        {
            Ok(Some(signals)) => {
                let mut stream_candles = candles.iter().skip(buffer_n_of_candles).enumerate();
                info!(
                    "Backtesting {} candles, with {} signals",
                    stream_candles.len(),
                    signals.len()
                );
                while let Some((index, candle)) = stream_candles.next() {
                    let signal = signals.get(index);
                    let signal: Option<Signal> = if signal.is_some() {
                        signal.unwrap().to_owned()
                    } else {
                        None
                    };
                    let _ = tx.send(MarketEvent {
                        time: candle.close_time,
                        asset: asset.clone(),
                        detail: MarketEventDetail::BacktestCandle((candle.to_owned(), signal)),
                    });
                }
            }
            Ok(None) => (),
            Err(e) => error!("Err on backtest: {:?}", e),
        };
    });

    Ok(rx)
}
