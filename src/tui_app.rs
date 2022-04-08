use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::{console::*, model::{ValidatedPullRequest, PursError, UserInputError, R, ValidSelection, NestedError}};

pub fn render_tui(items: Vec<ValidatedPullRequest>) -> R<ValidSelection> {
    // setup terminal
    enable_raw_mode().map_err(|e| PursError::TUIError(NestedError::from(e)))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(|e| PursError::TUIError(NestedError::from(e)))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| PursError::TUIError(NestedError::from(e)))?;

    // create app and run it
    let tick_rate = Duration::from_millis(250);
    let app = App::new(items);
    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode().map_err(|e| PursError::TUIError(NestedError::from(e)))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).map_err(|e| PursError::TUIError(NestedError::from(e)))?;
    terminal.show_cursor().map_err(|e| PursError::TUIError(NestedError::from(e)))?;

    // let _ = match res {
    //   Ok(value) => println!("{}", value),
    //   Err(err) => println!("{:?}", err)
    // };

    // Ok(())

    res
}


fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App<ValidatedPullRequest>,
    tick_rate: Duration,
) -> R<ValidSelection> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app)).map_err(|e| PursError::TUIError(NestedError::from(e)))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout).map_err(|e| PursError::TUIError(NestedError::from(e))) ? {
            if let Event::Key(key) = event::read().map_err(|e| PursError::TUIError(NestedError::from(e)))? {
                match key.code {
                    KeyCode::Char('q') => return Ok(ValidSelection::Quit),
                    KeyCode::Left => app.items.unselect(),
                    KeyCode::Down => app.items.next(),
                    KeyCode::Up => app.items.previous(),
                    KeyCode::Enter => {
                      let result = app.items.get_selected();
                      let selection_error = PursError::UserError(UserInputError::InvalidNumber("Could not match selected index".to_owned()));
                      return result.map(ValidSelection::Pr).ok_or(selection_error)
                    },
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            // app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App<ValidatedPullRequest>)
{
    // Create two chunks with equal horizontal screen space
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    // Iterate through all elements in the `items` app and append some debug text to it.
    let items: Vec<ListItem> = app
        .items
        .items
        .iter()
        .map(|i| {
            let lines = vec![Spans::from(i.to_string())];
            ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("List"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // We can now render the item list
    f.render_stateful_widget(items, chunks[0], &mut app.items.state);

    let selected =
      app.items
        .state.selected()
        .and_then(|i| {
          app.items
            .items
            .get(i)
            .map(|x| x)
        });

    // let text = vec![
    //     Spans::from(vec![
    //         Span::raw("First"),
    //         Span::styled("line",Style::default().fg(Color::Green).add_modifier(Modifier::ITALIC)),
    //         Span::raw("."),
    //     ]),
    //     Spans::from(Span::styled("Second line 🔥", Style::default().fg(Color::Red))),
    //     Spans::from(Span::styled(format!("Selected: {:?}", selected), Style::default().fg(Color::Red))),
    // ];

    let text =
      selected
        .clone()
        .map_or(
          no_pr_details("Select a PR to view its details"),
          |pr| pr_details(pr)
        );

    let p =
      Paragraph::new(text)
        .block(Block::default().title("Details").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment({
            match selected {
              Some(_) => Alignment::Left,
              None => Alignment::Center
            }
        })
        .wrap(Wrap { trim: true });

    // let banner = Text::raw("Select an item to display its details");
    f.render_widget(p, chunks[1]);
}


fn no_pr_details(message: &str) -> Vec<Spans> {
  let style = Style::default().fg(Color::Yellow);
  vec![
    Spans::from(
      vec![
      Span::styled(message, style)
      ]
    )
  ]
}

fn pr_details(pr: &ValidatedPullRequest) -> Vec<Spans> {

  let title = details_key_value("Title", pr.title.clone());
  let pr_no = details_key_value("PR#", pr.pr_number.to_string());
  let pr_url = details_key_value("Link", pr.ssh_url.to_string());
  let pr_repo = details_key_value("Repository", pr.repo_name.to_string());
  let pr_branch = details_key_value("Branch", pr.branch_name.to_string());


  vec![
    Spans::from(title),
    Spans::from(pr_no),
    Spans::from(pr_url),
    Spans::from(pr_repo),
    Spans::from(pr_branch),
  ]

}

fn details_key_value(key: &str, value: String) -> Vec<Span> {
  vec![
    Span::styled(key, Style::default().add_modifier(Modifier::BOLD)),
    Span::raw(": "),
    Span::styled(value, Style::default())
  ]
}