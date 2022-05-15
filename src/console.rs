use tui::{self, widgets};

pub struct StatefulList<T> {
  pub state: widgets::ListState,
  pub items: Vec<T>,
}


impl<T: Clone> StatefulList<T> {
  pub fn with_items(items: Vec<T>) -> StatefulList<T> {
      StatefulList {
        state: widgets::ListState::default(),
        items,
      }
  }

  pub fn next(&mut self) {
    let i = match self.state.selected() {
      Some(i) => {
        if i >= self.items.len() - 1 {
            0
        } else {
            i + 1
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }

  pub fn previous(&mut self) {
    let i = match self.state.selected() {
      Some(i) => {
        if i == 0 {
            self.items.len() - 1
        } else {
            i - 1
        }
      }
      None => 0,
    };
    self.state.select(Some(i));
  }

  pub fn unselect(&mut self) {
      self.state.select(None);
  }

  pub fn get_selected(&self) -> Option<T> {
      self.state
      .selected()
      .and_then(|i| self.items.get(i).cloned())
  }
}

pub struct App<T> {
    pub items: StatefulList<T>,
}

impl <T: Clone> App<T> {
    pub fn new(app_items: Vec<T>) -> App<T> {
        App {
          items: StatefulList::with_items(app_items)
        }
    }
}

