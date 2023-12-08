pub struct List<T> {
  items: Vec<T>,
  selected: Option<usize>,
}
impl<T> List<T> {
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

  fn draw() {}
}
impl<T> Default for List<T> {
  fn default() -> Self {
    List { items: Vec::new(), selected: None }
  }
}
