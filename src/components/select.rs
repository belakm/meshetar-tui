use super::style::{centered_rect, stylized_block};
use color_eyre::eyre::Result;
use ratatui::{
  prelude::{Alignment, Constraint, Layout, Margin, Rect},
  style::{Modifier, Style, Stylize},
  widgets::Paragraph,
  Frame,
};
use std::fmt::Display;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

pub struct Select<T> {
  items: Vec<T>,
  selected: usize,
  is_displayed: bool,
}
impl<T> Select<T>
where
  T: IntoEnumIterator + Display + Clone,
{
  pub fn is_displayed(&self) -> bool {
    self.is_displayed
  }

  pub fn next(&mut self) {
    let max_index = self.items.len() - 1;
    self.select(max_index.min(self.selected + 1));
  }

  pub fn previous(&mut self) {
    self.select(self.selected.saturating_sub(1));
  }

  pub fn select(&mut self, pos: usize) {
    self.selected = pos
  }

  pub fn open(&mut self) {
    self.is_displayed = true
  }

  pub fn close(&mut self) {
    self.is_displayed = false
  }

  pub fn selected(&self) -> T {
    self.items[self.selected].clone()
  }

  pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let layout = centered_rect(area.width - 4, self.items.len() as u16 + 2, area);
    f.render_widget(stylized_block(false), layout);
    let layout = layout.inner(&Margin { horizontal: 1, vertical: 1 });
    let constraints: Vec<Constraint> = self.items.iter().map(|_| Constraint::Length(1)).collect();
    let layout = Layout::default().constraints(constraints).split(layout);
    for (index, item) in self.items.iter().enumerate() {
      let mut paragraph = Paragraph::new(item.to_string()).alignment(Alignment::Center);
      if self.selected == index {
        paragraph = paragraph.add_modifier(Modifier::REVERSED);
      }
      f.render_widget(paragraph, layout[index]);
    }
    Ok(())
  }
}
impl<T> Default for Select<T>
where
  T: IntoEnumIterator,
{
  fn default() -> Self {
    Select { items: T::iter().collect(), selected: 0, is_displayed: false }
  }
}
