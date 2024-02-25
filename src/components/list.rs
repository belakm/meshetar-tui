use std::ops::Add;

use crate::strategy::ModelMetadata;

use super::ListDisplay;
use color_eyre::eyre::Result;
use ratatui::prelude::*;

pub struct List<T: ListDisplay + Clone + Default> {
  items: Vec<T>,
  selected: Option<usize>,
}

impl<T: ListDisplay + Clone + Default> List<T> {
  pub fn add(&mut self, item: T) {
    self.items.push(item);
  }

  pub fn next(&mut self) {
    self.select(Some(
      self
        .selected
        .unwrap_or(0)
        .saturating_add(1)
        .clamp(0, self.items.len().saturating_sub(1)),
    ));
  }

  pub fn previous(&mut self) {
    self.select(Some(self.selected.unwrap_or(0).saturating_sub(1)));
  }

  pub fn update_items(&mut self, items: Vec<T>) {
    self.items = items.clone()
  }

  pub fn unselect(&mut self) {
    self.select(None);
  }

  pub fn select(&mut self, pos: Option<usize>) {
    self.selected = pos
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn get_selected(&self) -> Option<T> {
    if let Some(selected) = self.selected {
      if selected < self.items.len() {
        Some(self.items[selected].clone())
      } else {
        None
      }
    } else {
      None
    }
  }

  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let layout = Layout::default()
      .constraints(vec![Constraint::Length(2), Constraint::Min(0)])
      .split(area);
    T::default().draw_header(f, layout[0])?;
    let item_height = 2;
    // Sub one item to all displayed for headers
    let n_drawable_items = (area.height / item_height).saturating_sub(1);
    let (start_index, end_index) = {
      if n_drawable_items >= self.items.len() as u16 {
        (0u16, (self.items.len().saturating_sub(1)) as u16)
      } else {
        (
          self
            .selected
            .unwrap_or(0)
            .clamp(0, self.items.len().saturating_sub(n_drawable_items as usize))
            as u16,
          self
            .selected
            .unwrap_or(0)
            .saturating_add(n_drawable_items as usize)
            .clamp(0, self.items.len().saturating_sub(1)) as u16,
        )
      }
    };
    let constraints: Vec<Constraint> =
      vec![Constraint::Length(2); n_drawable_items as usize];
    let list_layout = Layout::vertical(constraints).split(layout[1]);
    for (index, item) in self
      .items
      .iter_mut()
      .skip(start_index as usize)
      .take(n_drawable_items as usize)
      .enumerate()
    {
      let is_active =
        self.selected.unwrap_or(0).eq(&index.saturating_add(start_index.into()));
      item.draw(f, list_layout[index], is_active)?;
    }

    Ok(())
  }
}
impl<T: ListDisplay + Clone + Default> Default for List<T> {
  fn default() -> Self {
    List { items: Vec::new(), selected: Some(0) }
  }
}
