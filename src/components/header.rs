use std::default;

use chrono::{DateTime, Utc};
use color_eyre::eyre::Result;
use ratatui::{
  layout::{Alignment, Constraint, Layout, Rect},
  widgets::Paragraph,
  Frame,
};

use crate::utils::formatting::time_ago;

#[derive(Default)]
pub struct MeshetarHeader {
  btc_valuation: f64,
  usdt_valuation: f64,
  last_update: Option<DateTime<Utc>>,
  is_testnet: bool,
}

impl MeshetarHeader {
  pub fn new(is_testnet: bool) -> Self {
    Self { is_testnet, ..MeshetarHeader::default() }
  }
  pub fn last_updated(&self) -> Option<DateTime<Utc>> {
    self.last_update.clone()
  }
  pub fn update(&mut self, btc_valuation: f64, usdt_valuation: f64) {
    self.btc_valuation = btc_valuation;
    self.usdt_valuation = usdt_valuation;
    self.last_update = Some(Utc::now());
  }
  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let layout = Layout::horizontal(vec![
      Constraint::Length(24),
      Constraint::Length(1),
      Constraint::Min(0),
    ])
    .split(area);
    let info_layout = Layout::vertical(vec![
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(1),
    ])
    .split(layout[2]);
    f.render_widget(logo(), layout[0]);
    f.render_widget(
      Paragraph::new(self.btc_valuation.to_string() + " ₿").alignment(Alignment::Right),
      info_layout[0],
    );
    f.render_widget(
      Paragraph::new(self.usdt_valuation.to_string() + " $").alignment(Alignment::Right),
      info_layout[1],
    );

    let time = if self.last_update.is_some() {
      self.last_update.unwrap()
    } else {
      DateTime::default()
    };

    f.render_widget(
      Paragraph::new(time_ago(time)).alignment(Alignment::Right),
      info_layout[2],
    );
    Ok(())
  }
}

pub fn logo<'a>() -> Paragraph<'a> {
  let title = r#"╔╦╗╔═╗╔═╗╦ ╦╔═╗╔╦╗╔═╗╦═╗
║║║║╣ ╚═╗╠═╣║╣  ║ ╠═╣╠╦╝
╩ ╩╚═╝╚═╝╩ ╩╚═╝ ╩ ╩ ╩╩╚═"#;
  Paragraph::new(title).alignment(Alignment::Center)
}
