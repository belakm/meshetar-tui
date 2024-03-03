use super::{Screen, ScreenId};
use crate::{
  action::{Action, ScreenUpdate},
  components::{
    list::{LabelValueItem, List},
    style::{button, default_layout, logo, outer_container_block, stylized_block},
  },
  config::{Config, KeyBindings},
  database::{error::DatabaseError, Database},
  statistic::TradingSummary,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{
  mpsc::{UnboundedReceiver, UnboundedSender},
  Mutex,
};
use uuid::Uuid;

#[derive(Default)]
pub struct Report {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  short_report_list: Option<List<LabelValueItem<String>>>,
  core_id: Uuid,
}

impl Report {
  pub fn new(core_id: Uuid) -> Self {
    Self { core_id, ..Self::default() }
  }
}

impl Screen for Report {
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
    self.command_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(&mut self, config: Config) -> Result<()> {
    self.config = config;
    Ok(())
  }

  fn update(&mut self, action: Action) -> Result<Option<Action>> {
    match action {
      Action::Tick => {},
      Action::Accept => {
        if let Some(command_tx) = &self.command_tx {
          command_tx.send(Action::Navigate(ScreenId::SESSIONS))?;
        }
      },
      Action::ScreenUpdate(update) => match update {
        ScreenUpdate::Report(report) => {
          let mut list = List::default();
          list.update_items(report.generate_short_report());
          self.short_report_list = Some(list)
        },
        _ => {},
      },
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    f.render_widget(outer_container_block(), area);
    let inner_area = area.inner(&Margin { horizontal: 2, vertical: 2 });
    let (header_area, content_area) = default_layout(inner_area);
    f.render_widget(logo(), header_area);
    let content_layout = Layout::default()
      .constraints(vec![Constraint::Length(2), Constraint::Min(0), Constraint::Length(3)])
      .split(content_area);
    let button_layout = Layout::horizontal(vec![
      Constraint::Percentage(40),
      Constraint::Percentage(20),
      Constraint::Percentage(40),
    ])
    .split(content_layout[2]);
    f.render_widget(
      Paragraph::new("Report was generated in summary.html"),
      content_layout[0],
    );

    if let Some(short_report_list) = &mut self.short_report_list {
      short_report_list.draw(f, content_layout[1])?;
    }
    f.render_widget(button("Back", true), button_layout[1]);
    Ok(())
  }
}
