pub mod form;
pub mod list;
pub mod report;
pub mod select;
pub mod style;

use color_eyre::eyre::Result;
use ratatui::prelude::*;

pub trait Drawable {
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}
