use ratatui::{
  prelude::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

pub struct Theme {
  pub bg: Color,
  pub bg_button: Color,
  pub bg_button_selected: Color,
  pub bg_action_field: Color,
  pub bg_action_field_active: Color,
  pub bg_action_field_error: Color,
  pub border: Color,
  pub border_active: Color,
  pub text: Color,
  pub text_dimmed: Color,
  pub text_critical: Color,
  pub text_selected: Color,
  pub text_button: Color,
  pub text_button_selected: Color,
}

pub static DEFAULT_THEME: Theme = Theme {
  bg: Color::Indexed(234),
  bg_button: Color::Indexed(236),
  bg_button_selected: Color::Indexed(178),
  bg_action_field: Color::Indexed(236),
  bg_action_field_active: Color::Indexed(233),
  bg_action_field_error: Color::Indexed(52),
  border: Color::Indexed(250),
  border_active: Color::Green,
  text_dimmed: Color::Indexed(240),
  text: Color::Indexed(252),
  text_critical: Color::Red,
  text_selected: Color::White,
  text_button: Color::Indexed(252),
  text_button_selected: Color::Black,
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

pub fn default_style(active: bool) -> Style {
  if active {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.text_selected)
  } else {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.text)
  }
}

pub fn default_border_style(active: bool) -> Style {
  if active {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.border_active)
  } else {
    Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.border)
  }
}

pub fn default_action_block_style(active: bool, error: bool) -> Style {
  let text_style = if active { DEFAULT_THEME.text_selected } else { DEFAULT_THEME.text };
  if error {
    Style::default().bg(DEFAULT_THEME.bg_action_field_error).fg(text_style)
  } else if active {
    Style::default().bg(DEFAULT_THEME.bg_action_field_active).fg(text_style)
  } else {
    Style::default().bg(DEFAULT_THEME.bg_action_field).fg(text_style)
  }
}

pub fn header_style() -> Style {
  Style::default().bg(DEFAULT_THEME.bg).fg(DEFAULT_THEME.bg_button_selected)
}

pub fn stylized_button<'a>(selected: bool) -> Block<'a> {
  let border_style = button_border_style(selected);
  let content_style = button_style(selected);
  Block::default()
    .borders(Borders::ALL)
    .style(content_style)
    .border_style(border_style)
    .border_type(BorderType::Rounded)
}

pub fn button_style(selected: bool) -> Style {
  if selected {
    Style::default()
      .bg(DEFAULT_THEME.bg_button_selected)
      .fg(DEFAULT_THEME.text_button_selected)
      .add_modifier(Modifier::BOLD)
  } else {
    Style::default()
      .bg(DEFAULT_THEME.bg_button)
      .fg(DEFAULT_THEME.text_button)
      .add_modifier(Modifier::BOLD)
  }
}

pub fn button_border_style(selected: bool) -> Style {
  if selected {
    Style::default()
      .bg(DEFAULT_THEME.bg_button_selected)
      .fg(DEFAULT_THEME.bg_button_selected)
  } else {
    Style::default().bg(DEFAULT_THEME.bg_button).fg(DEFAULT_THEME.bg_button)
  }
}

pub fn outer_container_block<'a>() -> Block<'a> {
  let border_style = default_border_style(true);
  let content_style = default_style(true);
  Block::default()
    .borders(Borders::ALL)
    .style(content_style)
    .border_style(border_style)
    .border_type(BorderType::Rounded)
}

pub fn input_block<'a>(active: bool, error: bool) -> Block<'a> {
  Block::new().borders(Borders::BOTTOM).style(default_action_block_style(active, error))
}

pub fn default_layout(area: Rect) -> (Rect, Rect) {
  let layout = Layout::default()
    .constraints(vec![Constraint::Length(4), Constraint::Max(2), Constraint::Min(0)])
    .split(area);
  (layout[0], layout[2])
}

pub fn logo<'a>() -> Paragraph<'a> {
  let title = r#"╔╦╗╔═╗╔═╗╦ ╦╔═╗╔╦╗╔═╗╦═╗
║║║║╣ ╚═╗╠═╣║╣  ║ ╠═╣╠╦╝
╩ ╩╚═╝╚═╝╩ ╩╚═╝ ╩ ╩ ╩╩╚═"#;
  Paragraph::new(title).alignment(Alignment::Center).style(header_style())
}

pub fn default_header<'a>(text: &'a str) -> Paragraph<'a> {
  Paragraph::new(text)
    .alignment(Alignment::Center)
    .block(stylized_block(false).borders(Borders::BOTTOM))
}

pub fn centered_text<'a>(text: &'a str) -> Paragraph<'a> {
  Paragraph::new(text).alignment(Alignment::Center).block(stylized_block(false))
}

pub fn button<'a>(text: &'a str, is_selected: bool) -> Paragraph<'a> {
  Paragraph::new(text)
    .alignment(Alignment::Center)
    .block(Block::new().padding(Padding::vertical(1)).style(button_style(is_selected)))
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
