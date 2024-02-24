use color_eyre::eyre::Result;
use ratatui::{
  prelude::{Constraint, Direction, Layout, Margin, Rect},
  widgets::{Block, BorderType, Borders, Paragraph},
  Frame,
};

use crate::components::style::{default_action_block_style, input_block};

#[derive(Default)]
pub struct Input {
  label: String,
  value: f64,
  is_active: bool,
  is_editing: bool,
  has_error: bool,
}
impl Input {
  pub fn new(initial_value: Option<f64>, label: Option<String>) -> Self {
    Self {
      value: initial_value.unwrap_or(0f64),
      label: label.unwrap_or("".to_string()),
      is_active: false,
      has_error: false,
      is_editing: false,
    }
  }

  pub fn draw_edit(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    Ok(())
  }

  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    // Render container
    let input_area = Layout::vertical(vec![
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Min(0),
    ])
    .split(area.clone());

    let inner_input =
      Layout::horizontal(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(input_area[0]);

    // Render input area bottom line
    f.render_widget(
      Block::new()
        .borders(Borders::BOTTOM)
        .style(default_action_block_style(false, self.has_error)),
      input_area[1],
    );

    // Label
    f.render_widget(
      Paragraph::new(self.label.to_string())
        .block(input_block(self.is_active, self.has_error)),
      inner_input[0],
    );

    // Label
    f.render_widget(
      Paragraph::new(self.value.to_string())
        .block(input_block(self.is_active, self.has_error)),
      inner_input[1],
    );

    Ok(())
  }
  pub fn set_active(&mut self, val: bool) {
    self.is_active = val;
  }
  pub fn toggle_edit(&mut self) -> bool {
    self.is_editing = !self.is_editing;
    self.is_editing
  }
  pub fn set_error(&mut self) {
    self.has_error = true;
  }
  pub fn value(&self) -> f64 {
    self.value
  }
  pub fn set_value(&mut self, value: f64) {
    self.validate(value);
    self.value = value;
  }
  fn validate(&mut self, value: f64) {
    // TODO
  }
}
