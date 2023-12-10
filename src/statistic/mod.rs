pub mod dispersion;
pub mod error;
pub mod metric;
pub mod summary_drawdown;
pub mod summary_pnl;
pub mod welford_online;

use self::{
    metric::ratio::{CalmarRatio, SharpeRatio, SortinoRatio},
    summary_drawdown::DrawdownSummary,
    summary_pnl::{PnLReturnSummary, ProfitLossSummary},
};
use crate::{
    portfolio::position::Position,
    utils::formatting::{dt_to_readable, readable_duration},
};
use chrono::{DateTime, Utc};
use prettytable::{row, Cell, Row, Table};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct TradingSummary {
    pub pnl_returns: PnLReturnSummary,
    pub pnl: ProfitLossSummary,
    pub drawdown: DrawdownSummary,
    pub tear_sheet: TearSheet,
    pub starting_time: DateTime<Utc>,
}

impl TradingSummary {
    pub fn init(config: StatisticConfig, starting_time: Option<DateTime<Utc>>) -> Self {
        let starting_time = starting_time.unwrap_or_else(|| config.created_at);
        Self {
            pnl_returns: PnLReturnSummary::new(starting_time),
            pnl: ProfitLossSummary::new(),
            drawdown: DrawdownSummary::new(config.starting_equity),
            tear_sheet: TearSheet::new(config.risk_free_return),
            starting_time,
        }
    }
    pub fn update(&mut self, position: &Position) {
        self.pnl_returns.update(position);
        self.drawdown.update(position);
        self.tear_sheet.update(&self.pnl_returns, &self.drawdown);
        self.pnl.update(position);
    }
    pub fn generate_summary(&mut self, positions: &[Position]) {
        for position in positions.iter() {
            self.update(position)
        }
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct StatisticConfig {
    pub starting_equity: f64,
    pub trading_days_per_year: usize,
    pub risk_free_return: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
pub struct TearSheet {
    pub sharpe_ratio: SharpeRatio,
    pub sortino_ratio: SortinoRatio,
    pub calmar_ratio: CalmarRatio,
}

impl TearSheet {
    pub fn new(risk_free_return: f64) -> Self {
        Self {
            sharpe_ratio: SharpeRatio::init(risk_free_return),
            sortino_ratio: SortinoRatio::init(risk_free_return),
            calmar_ratio: CalmarRatio::init(risk_free_return),
        }
    }

    pub fn update(&mut self, pnl_returns: &PnLReturnSummary, drawdown: &DrawdownSummary) {
        self.sharpe_ratio.update(pnl_returns);
        self.sortino_ratio.update(pnl_returns);
        self.calmar_ratio
            .update(pnl_returns, drawdown.max_drawdown.drawdown.drawdown);
    }
}

impl TableBuilder for TearSheet {
    fn titles(&self) -> Row {
        row!["Sharpe Ratio", "Sortino Ratio", "Calmar Ratio"]
    }

    fn row(&self) -> Row {
        row![
            format!("{:.3}", self.sharpe_ratio.daily()),
            format!("{:.3}", self.sortino_ratio.daily()),
            format!("{:.3}", self.calmar_ratio.daily()),
        ]
    }
}

pub trait TableBuilder {
    fn titles(&self) -> Row;
    fn row(&self) -> Row;
    fn table(&self, id_cell: &str) -> Table {
        let mut table = Table::new();

        let mut titles = self.titles();
        titles.insert_cell(0, Cell::new(""));
        table.set_titles(titles);

        let mut row = self.row();
        row.insert_cell(0, Cell::new(id_cell));
        table.add_row(row);

        table
    }
    fn table_with<T: TableBuilder>(&self, id_cell: &str, another: (T, &str)) -> Table {
        let mut table = Table::new();
        let mut titles = self.titles();
        titles.insert_cell(0, Cell::new(""));
        table.set_titles(titles);

        let mut first_row = self.row();
        first_row.insert_cell(0, Cell::new(id_cell));
        table.add_row(first_row);

        let mut another_row = another.0.row();
        another_row.insert_cell(0, Cell::new(another.1));
        table.add_row(another_row);

        table
    }
}

pub fn combine(builders: Vec<(String, TradingSummary)>) -> Vec<Table> {
    let mut tables = vec![Table::new(), Table::new(), Table::new(), Table::new()];
    builders
        .into_iter()
        .enumerate()
        .for_each(|(row_index, (id, trading_summary))| {
            // Insert rows for each table
            tables[0].add_row(trading_summary.pnl_returns.row());
            tables[1].add_row(trading_summary.tear_sheet.row());
            tables[2].add_row(trading_summary.drawdown.row());
            tables[3].add_row(trading_summary.pnl.row());
            for table in tables.iter_mut() {
                table
                    .get_mut_row(row_index)
                    .unwrap()
                    .insert_cell(0, Cell::new(&id));
            }
            if row_index == 0 {
                let mut rows = Vec::with_capacity(4);
                rows.push(trading_summary.pnl_returns.titles());
                rows.push(trading_summary.tear_sheet.titles());
                rows.push(trading_summary.drawdown.titles());
                rows.push(trading_summary.pnl.titles());
                for (index, row) in rows.iter_mut().enumerate() {
                    //row.insert_cell(0, Cell::new("Asset"));
                    tables[index].set_titles(row.to_owned())
                }
            }
        });

    tables
}

pub fn exited_positions_table(positions: Vec<Position>) -> Table {
    let mut table = Table::new();
    let title_row = row![
        //"Asset",
        "position enter",
        "position exit",
        "Quantity",
        "Duration",
        "enter_fees_total",
        "exit_fees_total",
        "enter_value_gross",
        "exit_value_gross",
        "enter_avg_price_gross",
        "exit_avg_price_gross",
        "current_symbol_price",
        "realised_profit_loss",
        "unrealised_profit_loss",
        // "n_position_updates"
    ];
    table.set_titles(title_row);
    positions.iter().for_each(|position| {
        let duration = readable_duration(position.meta.enter_time, position.meta.update_time);
        let position_enter = dt_to_readable(position.meta.enter_time);
        let position_exit = dt_to_readable(position.meta.update_time);
        table.add_row(row![
            position.asset.to_string(),
            position_enter,
            position_exit,
            position.quantity,
            duration,
            position.enter_fees_total,
            position.exit_fees_total,
            position.enter_value_gross,
            position.exit_value_gross,
            position.enter_avg_price_gross,
            position.exit_avg_price_gross,
            position.current_symbol_price,
            position.realised_profit_loss,
            position.unrealised_profit_loss,
            //position.n_position_updates
        ]);
    });
    table
}
