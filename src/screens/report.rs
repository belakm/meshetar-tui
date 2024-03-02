use super::{Screen, ScreenId};
use crate::{
  action::Action,
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
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use uuid::Uuid;

#[derive(Default)]
pub struct Report {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  short_report_list: Option<List<LabelValueItem<String>>>,
  database: Option<Arc<Mutex<Database>>>,
  core_id: Uuid,
}

impl Report {
  pub fn new(database: Arc<Mutex<Database>>, core_id: Uuid) -> Self {
    Self { core_id, database: Some(database), ..Self::default() }
  }

  async fn sync_with_db(&mut self) -> Result<()> {
    if self.short_report_list.is_none() {
      let stats: Result<TradingSummary, DatabaseError> = {
        if let Some(db) = self.database.clone() {
          let mut db = db.try_lock()?;
          db.get_statistics(&self.core_id.clone())
        } else {
          Err(DatabaseError::DataMissing(format!(
            "Statistics for {} not found.",
            self.core_id.to_string()
          )))
        }
      };
      match stats {
        Ok(stats) => {
          let mut list = List::default();
          list.update_items(stats.generate_short_report());
          self.short_report_list = Some(list)
        },
        Err(e) => log::error!("{}", e.to_string()),
      }
    }
    Ok(())
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

  fn init(&mut self, area: Rect) -> Result<()> {
    let _ = self.sync_with_db();
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
    .split(content_layout[1]);
    f.render_widget(
      Paragraph::new("Report was generated in summary.html"),
      content_layout[0],
    );

    if let Some(mut short_report_list) = self.short_report_list {
      short_report_list.draw(f, content_layout[1]);
    }
    f.render_widget(button("Back", true), button_layout[1]);
    Ok(())
  }
}
