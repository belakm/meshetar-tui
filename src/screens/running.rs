use super::Screen;
use crate::{
  action::Action,
  components::style::stylized_block,
  config::{Config, KeyBindings},
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
pub struct Running {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
}

impl Running {
  pub fn new() -> Self {
    Self::default()
  }
}

impl Screen for Running {
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
      _ => {},
    }
    Ok(None)
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let container = stylized_block(false);
    f.render_widget(container, area);
    f.render_widget(Paragraph::new("hello world"), area.inner(&Margin { horizontal: 2, vertical: 2 }));
    Ok(())
  }
}
