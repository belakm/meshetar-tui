#%%
import pandas as pd
import numpy as np
from ta import add_all_ta_features
from ta.utils import dropna
import matplotlib.pyplot as plt
import sqlite3
from scipy.signal import find_peaks
import seaborn as sns
import tensorflow as tf
from tensorflow.keras.callbacks import LearningRateScheduler
from scipy.special import softmax
from sklearn.preprocessing import StandardScaler, LabelEncoder, RobustScaler
from sklearn.metrics import roc_curve, roc_auc_score, RocCurveDisplay, confusion_matrix
from sklearn.model_selection import train_test_split
import pickle
import warnings
import os
pd.set_option('display.max_rows', 500)
pd.set_option('display.max_columns', 50)
#while not os.path.basename(os.getcwd()) == 'meshetar/server':
#    os.chdir('..')  # Move up one directory

warnings.filterwarnings("ignore", category=RuntimeWarning)


# %%
#import os
#print(os.path.abspath('./database.sqlite'))
conn = sqlite3.connect('./database.sqlite')
# cursor = sqliteConnection.cursor()
query = """SELECT open_time,
                open,
                 high, 
                 low, 
                 close, 
                 volume
          FROM candles 
          WHERE asset = 'BTCUSDT'"""

# %%
klines = pd.read_sql_query(query, conn)
klines['open_time'] = pd.to_datetime(klines['open_time'])
klines.loc[:, klines.columns.difference(['open_time'])] = klines.loc[:, klines.columns.difference(['open_time'])].apply(pd.to_numeric, errors='coerce')
#%%
klines = add_all_ta_features(klines,
                             open = "open", 
                             close = "close",
                             volume = "volume",
                             low = "low",
                             high = "high",
                             fillna=True).dropna()
# %%
one_percent_of_klines = klines.shape[0]*0.01
peaks, _ = find_peaks(klines["close"], 
                      distance = one_percent_of_klines)
peaks = np.append(peaks, peaks + 1)

klines["close"] = klines["close"].astype(float)
valleys, _ = find_peaks(-klines["close"], 
                         distance = one_percent_of_klines)
valleys = np.append(valleys, valleys + 1)


# %%
klines["signal"] = "hold"
klines.loc[valleys, 'signal'] = "buy"
klines.loc[peaks, 'signal'] = "sell"

# %%
columns_not_to_predict = [
    'open_time', 
    'open',
    'close',
    'low', 
    'high', 
    'volume',
    'signal']
klines_to_predict = klines.drop(columns=columns_not_to_predict)
# %%
# lags = range(1, 3)

# klines_to_predict= pd.concat([
#     klines_to_predict.assign(**{f'{col} (t-{lag})': klines_to_predict[col].shift(lag)})
#     for lag in lags
#     for col in klines_to_predict
# ], axis=1)

# %%
X_train , X_test, y_train , y_test = train_test_split(
    klines_to_predict, 
    klines[['signal']], 
    test_size=0.2,  
    shuffle=False)
# %%
# y_train['target'] = y_train['signal'].idxmax(axis = 0)
# y_test['target'] = y_test.idxmax(axis=0)
# %%
label_encoder = LabelEncoder()
y_train['target_encoded'] = label_encoder.fit_transform(y_train.values.ravel())
y_test['target_encoded'] = label_encoder.transform(y_test.values.ravel())
# %%
scaler = RobustScaler()
X_train = scaler.fit_transform(X_train)
X_test = scaler.fit_transform(X_test)
#%%
def schedule(epoch):
    if epoch < 10:
        return 3e-4
    return 1e-4 if epoch < 25 else 3e-5
scheduler = LearningRateScheduler(schedule)
#%%
model = tf.keras.Sequential([
    tf.keras.layers.Input(shape=(X_train.shape[1],)),
    tf.keras.layers.Dense(256, activation='relu'),
    tf.keras.layers.Dense(164, activation='relu'),
    tf.keras.layers.Dense(32, activation='relu'),
    tf.keras.layers.Dense(len(label_encoder.classes_), activation='softmax')
])
model.summary()

#%%55
model.compile(
    optimizer='adam',
    loss='sparse_categorical_crossentropy', 
)
# %%
class_weights = len(y_train)/(3*y_train['target_encoded'].value_counts())
normalized_class_weights = {cls: weight / sum(class_weights) for cls, weight in class_weights.items()}
#%%
decay_rate = 0.0005
sample_weights = np.exp(-decay_rate * (len(y_train) - y_train.index))
# plt.plot(sample_weights)
#%%
history = model.fit(
    X_train, 
    y_train['target_encoded'],
    epochs=100, 
    batch_size=86,
    # batch_size=86,
    validation_data=(X_test, y_test['target_encoded']),
    callbacks=[scheduler],
    sample_weight=sample_weights,
    # class_weight=normalized_class_weights,
)
#%%
plt.plot(history.history["loss"])
plt.plot(history.history["val_loss"])
plt.show()
#%%
train_proba = model.predict(X_train)
test_proba = model.predict(X_test)
# %%
cutoffs = []  # Create an empty list to store the cutoffs
for class_index in range(len(label_encoder.classes_)):
    fpr_train, tpr_train, thresholds_train = roc_curve(y_train['target_encoded'] == class_index, train_proba[:, class_index])
    # roc = RocCurveDisplay.from_predictions(y_train['target_encoded'] == class_index, train_proba[:, class_index])
    # roc.plot()
    # plt.title(f"Train roc for class: {class_index}")
    # plt.show()

    fpr_test, tpr_test, thresholds_test = roc_curve(y_test['target_encoded'] == class_index, test_proba[:, class_index])
    # roc = RocCurveDisplay.from_predictions(y_test['target_encoded'] == class_index, test_proba[:, class_index])
    # roc.plot()
    # plt.title(f"Test roc for class: {class_index}")
    # plt.show()

    optimal_cutoff_index = np.argmax(tpr_train - fpr_train)
    optimal_cutoff = thresholds_train[optimal_cutoff_index]

    cutoffs.append(optimal_cutoff)  # Append the class index and optimal cutoff as a tuple    print(f"{class_index, optimal_cutoff =}")

    y_train[f'model_prediction_V{class_index+1}'] = list(zip(*train_proba))[class_index] > optimal_cutoff
    y_test[f'model_prediction_V{class_index+1}'] = list(zip(*test_proba))[class_index] > optimal_cutoff
 
 
# %%
cutoffs[0] = cutoffs[0] + 0.02
cutoffs[2] = cutoffs[2] + 0.02

with open('./models/cutoffs.pickle', 'wb') as handle:
    pickle.dump(cutoffs, handle, protocol=-1)
print(test_proba)
print(cutoffs)
# %%
def set_model_prediction(row):
    if row["model_prediction_V1"]:
        return "buy"
    elif row["model_prediction_V3"]:
        return "sell"
    else:
        return "hold"
y_train['model_prediction'] = y_train.apply(set_model_prediction, axis=1).astype(str)
y_test['model_prediction'] = y_test.apply(set_model_prediction, axis=1).astype(str)

# %%
conf_matrix = confusion_matrix(y_train['signal'], y_train['model_prediction'], labels=label_encoder.classes_)
# plt.figure(figsize=(8, 6))
# sns.heatmap(conf_matrix, annot=True, fmt='d', cmap='Blues', xticklabels=label_encoder.classes_, yticklabels=label_encoder.classes_)
# plt.xlabel('Model Prediction')
# plt.ylabel('Actual Target')
# plt.title('Confusion Matrix train')
# plt.show()
# %%
conf_matrix = confusion_matrix(y_test['signal'], y_test['model_prediction'], labels=label_encoder.classes_)
# plt.figure(figsize=(8, 6))
# sns.heatmap(conf_matrix, annot=True, fmt='d', cmap='Blues', xticklabels=label_encoder.classes_, yticklabels=label_encoder.classes_)
# plt.xlabel('Model Prediction')
# plt.ylabel('Actual Target')
# plt.title('Confusion Matrix test')
# plt.show()

# %%
model.save("./models/neural_net_model")
#
# %%
test_set_close = klines.loc[y_test.index, 'close']
test_set_open_time = klines.loc[y_test.index, 'open_time']

plt.plot(test_set_close, label='Actual Close Prices', color='blue')
plt.plot(test_set_close.index[y_test['model_prediction'] == "buy"], 
         test_set_close[y_test['model_prediction'] == "buy"],
         'go', label='Buy', markersize=3)
plt.plot(test_set_close.index[y_test['model_prediction'] == "sell"], 
         test_set_close[y_test['model_prediction'] == "sell"],
         'ro', label='sell', markersize=3)
plt.title('Buy and Sell Predictions vs. Actual Close Prices')
plt.legend()
plt.grid(True)
plt.show()

plt.plot( klines["close"])
plt.plot(peaks, klines["close"][peaks], "x", color = "red")
plt.plot(valleys, klines["close"][valleys], "+", color = "green")
plt.savefig('./static/historic_signals.svg', format='svg')


# %%
# Backtesting
back_test = pd.DataFrame(
    {'close': test_set_close,
     'open_time': test_set_open_time,
     'returns' : np.log(test_set_close/test_set_close.shift(1)),
     'predicted_signal':  y_test['model_prediction']})
back_test = back_test.reset_index()

back_test['position'] = back_test['predicted_signal'].replace(to_replace="hold", method='ffill')
back_test['position'] = back_test['position'].shift(1)
# Create an initial balance (starting balance)
initial_balance = 1000
back_test.at[0, 'balance'] = initial_balance
print(back_test)
# %%
current_stake = 0
current_balance = initial_balance
asset_quantity = 0



for index, row in back_test.iterrows():
    if index == 0: print(f"first candle: {row['open_time']}")
    if row['position'] == 'buy' and current_balance > 0:
        asset_quantity = current_balance / row['close']  # Buy as much as possible
        current_balance = 0
        print(f"buy at {row.open_time}")
    elif row['position'] == 'sell' and asset_quantity > 0:
        current_balance = asset_quantity * row['close']  # Sell all assets
        asset_quantity = 0
        print(f"sell at {row.open_time}")
    back_test.at[index, 'balance'] = current_balance + (asset_quantity * row['close'])

# # Final balance accounting for any unsold assets
final_balance = current_balance + (asset_quantity * back_test['close'].iloc[-1])
print(f"Final balance: {final_balance:.2f}")

# for index, row in back_test.iterrows():
#     if (index == 0):
#         print(f"Start time: {row.open_time}")
#     if row['position'] == 'buy' and current_balance == 0:
#         back_test.at[index, 'balance'] = 0
#         continue
#     elif row['position'] == 'buy' and current_balance != 0:
#         print(f"buy at {row.open_time} price {row.close}")
#         back_test.at[index, 'balance'] = 0
#         current_stake = current_balance
#         current_balance = 0
#         close_price_at_buy = back_test.at[index, 'close']
#     elif row['position'] == 'sell' and current_stake == 0:
#         back_test.at[index, 'balance'] = current_balance
#         continue
#     elif row['position'] == 'sell' and current_stake != 0:
#         if index + 1 < len(back_test):
#             print(f"sell at {row.open_time} price {row.close}")
#             current_stake = row['close']-close_price_at_buy + current_stake
#             back_test.at[index, 'balance'] = current_stake
#             current_balance = current_stake
#             current_stake = 0
#
# last_nonzero = back_test[back_test['balance']!= 0].iloc[-1]['balance']
# last_nonzero
#
# # %%
# buy_and_sell_scenario = back_test['close'].iloc[-1] - back_test['close'].iloc[0]
# print(f"If we would buy and sell after complete backtest period, change is {buy_and_sell_scenario:.1f}€")
#
# # %%
# print(f"""Starting close price: {back_test['close'].iloc[0]:.1f}€,
#     Ending close price: {back_test['close'].iloc[-1]:.1f}€""")
# # %%
# last_nonzero = back_test[back_test['balance']!= 0].iloc[-1]['balance']
# balance_difference = last_nonzero - back_test['balance'].iloc[0] 
# pct_change = (last_nonzero/initial_balance)*100
# print(f"From {initial_balance}€, final balance is: {last_nonzero:.0f}€, which is {pct_change:.3f}%")
# # %%
