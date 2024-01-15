use color_eyre::eyre::Result;
use ratatui::{
  prelude::{Constraint, Layout, Margin, Rect},
  widgets::{Block, BorderType, Borders, Paragraph},
  Frame,
};

use crate::components::style::default_action_block_style;

#[derive(Default)]
pub struct Input {
  label: String,
  value: i64,
  is_active: bool,
  has_error: bool,
}
impl Input {
  pub fn new(initial_value: Option<i64>, label: Option<String>) -> Self {
    Self {
      value: initial_value.unwrap_or(0),
      label: label.unwrap_or("".to_string()),
      is_active: false,
      has_error: false,
    }
  }
  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    // Render container
    let input_area = Layout::new()
      .constraints(vec![Constraint::Length(1), Constraint::Length(2), Constraint::Min(0)])
      .split(area);

    f.render_widget(
      Block::new().style(default_action_block_style(self.is_active, self.has_error)),
      input_area[1],
    );

    // Render input area bottom line
    f.render_widget(
      Block::new()
        .borders(Borders::BOTTOM)
        .style(default_action_block_style(self.is_active, self.has_error)),
      input_area[1].inner(&Margin { horizontal: 1, vertical: 0 }),
    );

    // Label
    f.render_widget(
      Paragraph::new(self.label.to_string()).block(
        Block::new().style(default_action_block_style(self.is_active, self.has_error)),
      ),
      input_area[0],
    );

    let value_area = input_area[1].inner(&Margin { horizontal: 2, vertical: 0 });
    // Value
    f.render_widget(
      Paragraph::new(self.value.to_string()).block(
        Block::new().style(default_action_block_style(self.is_active, self.has_error)),
      ),
      value_area,
    );
    Ok(())
  }
  pub fn set_active(&mut self, val: bool) {
    self.is_active = val;
  }
  pub fn set_error(&mut self) {
    self.has_error = true;
  }
  pub fn set_value(&mut self, value: i64) {
    self.validate(value);
    self.value = value;
  }
  fn validate(&mut self, value: i64) {
    // TODO
  }
}
