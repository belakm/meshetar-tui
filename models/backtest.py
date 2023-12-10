#%%
import tensorflow as tf
import sqlite3
import pandas as pd
from ta import add_all_ta_features
import warnings
import pickle
from sklearn.preprocessing import RobustScaler
import os
while not os.path.basename(os.getcwd()) == 'server':
    os.chdir('..')  # Move up one directory

def backtest(candle_time=None, pair="BTCUSDT"):
    warnings.simplefilter(action='ignore', category=FutureWarning)
    warnings.simplefilter("ignore", category=RuntimeWarning)

    #%%

    # Load the saved model
    loaded_model = tf.keras.models.load_model("./models/neural_net_model")  # Specify the path to your saved model directory or .h5 file

    #%%
    conn = sqlite3.connect('./database.sqlite')
    time_query = f"AND open_time >= \"{candle_time}\"" if candle_time else ""
    print(time_query)
    # cursor = sqliteConnection.cursor()
    query = f"""
    SELECT open_time,
    open,
    high, 
    low, 
    close, 
    volume
    FROM candles
    WHERE asset = '{pair}'
    {time_query}
    ORDER BY open_time ASC 
    LIMIT 1440;"""

    klines = pd.read_sql_query(query, conn)
    # Make predictions using the loaded model
    klines = add_all_ta_features(klines,
                                    open = "open", 
                                    close = "close",
                                    volume = "volume",
                                    low = "low",
                                    high = "high",
                                    fillna=True).dropna()
    #%%

    # Store the 'open_time' column
    open_time_data = klines['open_time'].copy()
    close_data = klines['close'].copy()


    columns_not_to_predict = [
        'open_time', 
        'open',
        'close', 
        'low', 
        'high', 
        'volume']
    klines_to_predict = klines.drop(columns=columns_not_to_predict)

    # lags = range(1, 15)
    # klines_to_predict.assign(**{
    #     f'{col} (t-{lag})': klines_to_predict[col].shift(lag)
    #     for lag in lags
    #     for col in klines_to_predict
    # })

    #%%
    scaler = RobustScaler()
    klines_to_predict = scaler.fit_transform(klines_to_predict.astype('float32'))
    predictions = loaded_model.predict(klines_to_predict)
    print(klines)
    file_path = './models/cutoffs.pickle'
    with open(file_path, 'rb') as handle:
        cutoffs = pickle.load(handle)
    print(cutoffs)
    cut_predictions = pd.DataFrame()
    for index, cutoff in enumerate(cutoffs):  
        cut_predictions[f'model_prediction_V{index+1}']=  list(zip(*predictions))[index] > cutoff  
    def set_model_prediction(row):
        if row["model_prediction_V1"]:
            return "buy"
        elif row["model_prediction_V3"]:
            return "sell"
        else:
            return "hold"
    cut_predictions['model_prediction'] = cut_predictions.apply(set_model_prediction, axis=1).astype(str)

    # Merge 'open_time' with the predictions
    cut_predictions['open_time'] = open_time_data.reset_index(drop=True)
    cut_predictions['close'] = close_data.reset_index(drop=True)
    
    # Return combined data
    combined_predictions = list(zip(cut_predictions['open_time'], cut_predictions['model_prediction'], cut_predictions['close']))
    #combined_predictions = list(zip(cut_predictions['open_time'], cut_predictions['model_prediction'])) 

    # Revert order
    return combined_predictions

predictions = backtest("2023-12-01T12:50:00+00:00")

initial_balance = 1000
current_balance = initial_balance
current_stake = 0
close_price_at_buy = 0

for time, action, close in predictions:
    if action == 'buy' and current_stake == 0:
        print(f"Buy at {time} price {close}")
        # Spend the entire current balance to buy at the close price
        current_stake = current_balance / close
        current_balance = 0
        close_price_at_buy = close
        print(current_stake * close)
    elif action == 'sell' and current_stake != 0:
        print(f"Sell at {time} price {close}")
        # Sell all the stake at the current close price
        current_balance = current_stake * close
        print(current_balance)
        current_stake = 0

# If the final action was a buy, sell at the last price to realize the profit/loss
if current_stake != 0:
    current_balance = current_stake * predictions[-1][2]
    current_stake = 0

final_balance = current_balance
print(f"Initial balance: {initial_balance}")
print(f"Final balance: {final_balance}")
