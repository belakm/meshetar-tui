pub mod form;
pub mod header;
pub mod list;
pub mod report;
pub mod style;

use eyre::Result;
use ratatui::prelude::*;

pub trait ListDisplay {
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect, active: bool) -> Result<()>;
  fn draw_header(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}
