# %%
import binance
import pandas as pd
import datetime as dt
import time
import toml
import sqlite3
import os
while not os.path.basename(os.getcwd()) == 'server':
    os.chdir('..')  # Move up one directory
#%%
# Read and parse the TOML configuration file
config = toml.load('./config.toml')
client =  binance.Client(config["binance_api_key"],
                config["binance_api_secret"],
                testnet=True)
# %%
def get_historical_ohlc_data(symbol,past_days=None,interval=None):
    
    """Returns historcal klines from past for given symbol and interval
    past_days: how many days back one wants to download the data"""
    
    if not interval:
        interval='1h' # default interval 1 hour
    if not past_days:
        past_days=30  # default past days 30.

    start_str=str((pd.to_datetime('today')-pd.Timedelta(str(past_days)+' days')).date())
    
    D=pd.DataFrame(client.get_historical_klines(symbol=symbol,start_str=start_str,interval=interval))
    D.columns=['open_time','open', 'high', 'low', 'close', 'volume', 'close_time', 'qav', 'num_trades', 'taker_base_vol', 'taker_quote_vol','is_best_match']
    D['open_date_time']=[dt.datetime.fromtimestamp(x/1000) for x in D.open_time]
    D['symbol']=symbol
    D=D[['symbol','open_date_time','open', 'high', 'low', 'close', 'volume', 'num_trades', 'taker_base_vol', 'taker_quote_vol']]

    return D
# %%
klines = get_historical_ohlc_data("BTCUSDT",  past_days=30, interval= "5m")

klines["open_time"] = pd.to_datetime(klines["open_date_time"])
klines['symbol'] = klines['symbol'].astype(str)
# Convert 'open,' 'high,' 'low,' 'close,' 'volume,' 'taker_base_vol,' and 'taker_quote_vol' to numeric (float)
columns_to_convert = ['open', 'high', 'low', 'close', 'volume', 'taker_base_vol', 'taker_quote_vol']
klines[columns_to_convert] = klines[columns_to_convert].apply(pd.to_numeric, errors='coerce')
klines.rename(columns={'symbol': 'asset'}, inplace=True)
# %%
conn = sqlite3.connect('database.sqlite')
cursor = conn.cursor()
klines.to_sql('candles', conn, if_exists='replace', index=False)
conn.close()
# %%
