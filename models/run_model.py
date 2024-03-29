import os
# THIS MUTES TENSORFLOW 
os.environ['TF_CPP_MIN_LOG_LEVEL'] = '3'

#%%
import tensorflow as tf
import sqlite3
import pandas as pd
from ta import add_all_ta_features
import warnings
import pickle
from sklearn.preprocessing import RobustScaler
import sys

def suppress_output():
    sys.stdout = open(os.devnull, 'w')
    sys.stderr = open(os.devnull, 'w')

def run(candle_time=None, pair="BTCUSDT", model_name="neural_net_model"):
    from random import choices
    return choices(["hold", "buy", "sell"], [0.6, 0.2, 0.2])[0]

def run2(candle_time=None, pair="BTCUSDT", model_name="neural_net_model"):
    # Comment out the warning silencers below when developing:
    warnings.simplefilter(action='ignore', category=FutureWarning)
    warnings.simplefilter("ignore", category=RuntimeWarning)
    warnings.simplefilter(action='ignore', category=pd.errors.PerformanceWarning)
    suppress_output()

    # Load the saved model
    model_path = "./models/generated/" + model_name;
    loaded_model = tf.keras.models.load_model(model_path)  # Specify the path to your saved model directory or .h5 file
    
    conn = sqlite3.connect('./database.sqlite')
    # cursor = sqliteConnection.cursor()
    time_query = f"AND open_time <= \"{candle_time}\"" if candle_time else ""

    query = f"""
    SELECT open_time,
    open,
    high, 
    low, 
    close, 
    volume
    FROM candles
    WHERE asset = '{pair}'
    AND volume > 0
    {time_query}
    ORDER BY open_time DESC
    LIMIT 50;"""
    
    klines = pd.read_sql_query(query, conn)
   
    # Make predictions using the loaded model
    klines = add_all_ta_features(klines,
                                 open = "open", 
                                 close = "close",
                                 volume = "volume",
                                 low = "low",
                                 high = "high",
                                 fillna=True)
    
    columns_not_to_predict = [
        'open_time', 
        'open',
        'close', 
        'low', 
        'high', 
        'volume']
    klines_to_predict = klines.drop(columns=columns_not_to_predict)
    scaler = RobustScaler()
    klines_to_predict = scaler.fit_transform(klines_to_predict.astype('float32'))
    predictions = loaded_model.predict(klines_to_predict)
    pickle_path = model_path + '/cutoffs.pickle'
    with open(pickle_path, 'rb') as handle:
        cutoffs = pickle.load(handle)

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
    result = set_model_prediction(cut_predictions.iloc[-1])
    print(result)
    return result
