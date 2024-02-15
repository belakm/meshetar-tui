use super::Drawable;
use color_eyre::eyre::Result;
use ratatui::prelude::*;

pub struct List<T: Drawable> {
  items: Vec<T>,
  selected: Option<usize>,
}

impl<T: Drawable> List<T> {
  fn add(&mut self, item: T) {
    self.items.push(item);
  }

  fn next(&mut self) {
    let i = match self.selected {
      Some(i) => {
        if i >= self.items.len() - 1 {
          None
        } else {
          Some(i + 1)
        }
      },
      None => Some(0),
    };
    self.select(i);
  }

  fn previous(&mut self) {
    let i = match self.selected {
      Some(i) => {
        if i == 0 {
          None
        } else {
          Some(i - 1)
        }
      },
      None => None,
    };
    self.select(i);
  }

  fn update_items(&mut self, items: Vec<T>) {
    self.items = items
  }

  fn unselect(&mut self) {
    self.select(None);
  }

  fn select(&mut self, pos: Option<usize>) {
    self.selected = pos
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let item_height = 3;
    let n_drawable_items = area.height / item_height;
    let start_index: u16 = self.selected.unwrap_or(0) as u16;
    let end_index: u16 = (start_index + n_drawable_items - 1)
      .max(self.items.len() as u16 - n_drawable_items + 1);
    let constraints: Vec<Constraint> = self
      .items
      .iter()
      .skip(n_drawable_items as usize)
      .map(|_| Constraint::Length(2))
      .collect();
    let list_layout = Layout::new().constraints(constraints).split(area);

    for (index, item) in self
      .items
      .iter_mut()
      .skip(start_index as usize)
      .take(n_drawable_items as usize)
      .enumerate()
    {
      item.draw(f, list_layout[index])?;
    }

    Ok(())
  }
}
impl<T: Drawable> Default for List<T> {
  fn default() -> Self {
    List { items: Vec::new(), selected: None }
  }
}
