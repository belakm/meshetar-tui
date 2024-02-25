use color_eyre::eyre::Result;
use ratatui::{
  prelude::{Constraint, Direction, Layout, Margin, Rect},
  style::{Color, Style, Stylize},
  widgets::{Block, BorderType, Borders, Clear, Paragraph},
  Frame,
};
use std::fmt::Display;
use uuid::Uuid;

use crate::{
  assets::Pair,
  components::{
    list::List,
    style::{default_action_block_style, input_block, stylized_block},
    ListDisplay,
  },
  strategy::ModelId,
};

impl ListDisplay for ModelId {
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect, active: bool) -> Result<()> {
    let layout =
      Layout::horizontal(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    f.render_widget(
      Paragraph::new(self.pair.to_string()).block(input_block(active, false)),
      layout[0],
    );
    f.render_widget(
      Paragraph::new(self.name.clone()).block(input_block(active, false)),
      layout[1],
    );
    Ok(())
  }
  fn draw_header(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let layout =
      Layout::horizontal(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    f.render_widget(
      Paragraph::new("Pair".to_string()).block(input_block(false, false)),
      layout[0],
    );
    f.render_widget(
      Paragraph::new("Pet name".to_string()).block(input_block(false, false)),
      layout[1],
    );
    Ok(())
  }
}

impl ListDisplay for Pair {
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect, active: bool) -> Result<()> {
    f.render_widget(
      Paragraph::new(self.to_string()).block(input_block(active, false)),
      area,
    );
    Ok(())
  }
  fn draw_header(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    Ok(())
  }
}

#[derive(Default)]
pub struct Select<T: Display + Clone + ListDisplay + Default> {
  label: String,
  value: Option<T>,
  options: Vec<T>,
  is_active: bool,
  is_editing: bool,
  has_error: bool,
  edit_list: List<T>,
  edit_list_index: usize,
}
impl<T: Display + Clone + ListDisplay + Default> Select<T> {
  pub fn new(options: Vec<T>, value: Option<T>, label: Option<String>) -> Self {
    let mut edit_list = List::default();
    edit_list.update_items(options.clone());
    Self {
      value,
      options,
      label: label.unwrap_or("".to_string()),
      is_active: false,
      is_editing: false,
      has_error: false,
      edit_list,
      edit_list_index: 0,
    }
  }

  pub fn draw_edit(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    if self.is_editing {
      let layout = Layout::vertical(vec![
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
      ])
      .split(area);
      let inner_layout = Layout::horizontal(vec![
        Constraint::Percentage(10),
        Constraint::Min(0),
        Constraint::Percentage(10),
      ])
      .split(layout[1]);
      f.render_widget(Clear, inner_layout[1]);
      f.render_widget(input_block(false, false), inner_layout[1]);
      self
        .edit_list
        .draw(f, inner_layout[1].inner(&Margin { horizontal: 1, vertical: 0 }))?;
    }

    Ok(())
  }

  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
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

    // Value
    let value = if self.value.is_some() {
      self.value.clone().unwrap().to_string()
    } else {
      "None".to_string()
    };
    f.render_widget(
      Paragraph::new(value).block(input_block(self.is_active, self.has_error)),
      inner_input[1],
    );

    Ok(())
  }
  pub fn set_active(&mut self, val: bool) {
    self.is_active = val;
  }
  pub fn toggle_edit(&mut self) -> bool {
    if self.is_editing {
      self.value = self.edit_list.get_selected();
    }
    self.is_editing = !self.is_editing;
    self.is_editing
  }
  pub fn set_error(&mut self) {
    self.has_error = true;
  }
  pub fn value(&self) -> Option<T> {
    self.value.clone()
  }
  pub fn set_value(&mut self, value: Option<T>) {
    self.validate(value.clone());
    self.value = value;
  }
  pub fn edit_next(&mut self) {
    self.edit_list.next();
  }
  pub fn edit_previous(&mut self) {
    self.edit_list.previous();
  }
  pub fn set_options(&mut self, items: Vec<T>) {
    self.edit_list.update_items(items);
  }
  fn has_no_options(&self) -> bool {
    self.edit_list.is_empty()
  }
  fn validate(&mut self, value: Option<T>) {
    // TODO
  }
}
