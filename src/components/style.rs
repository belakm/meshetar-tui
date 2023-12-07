use ratatui::{
  prelude::{Constraint, Direction, Layout, Rect},
  style::{Color, Style},
  widgets::{Block, BorderType, Borders},
};

pub struct Theme {
  bg: Color,
  border: Color,
  border_selected: Color,
  text: Color,
  text_selected: Color,
}

static DEFAULT_THEME: Theme = Theme {
  bg: Color::Indexed(234), // dark-grey
  border: Color::Indexed(250),
  text: Color::Indexed(252),
  border_selected: Color::Green,
  text_selected: Color::White,
};

pub fn stylized_block<'a>(selected: bool) -> Block<'a> {
  let border_style = default_border_style(selected);
  let content_style = default_style(selected);
  Block::default()
    .borders(Borders::ALL)
    .style(content_style)
    .border_style(border_style)
    .border_type(BorderType::Rounded)
}

pub fn default_style(selected: bool) -> Style {
  if selected {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.text_selected)
  } else {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.text)
  }
}

pub fn default_border_style(selected: bool) -> Style {
  if selected {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.border_selected)
  } else {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.border)
  }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect_procentage(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage((100 - percent_y) / 2),
      Constraint::Percentage(percent_y),
      Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Percentage((100 - percent_x) / 2),
      Constraint::Percentage(percent_x),
      Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
  let popup_layout = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length((r.width - width) / 2),
      Constraint::Length(width),
      Constraint::Length((r.width - width) / 2),
    ])
    .split(r);

  Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length((r.height - height) / 2),
      Constraint::Length(height),
      Constraint::Length((r.height - height) / 2),
    ])
    .split(popup_layout[1])[1]
}
