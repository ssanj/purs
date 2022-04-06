use crossterm::{
    // event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    event::{self, Event, KeyCode},
    // execute,
    // terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use tui::{
    // backend::{Backend, CrosstermBackend},
    backend::Backend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::console::*;


fn run_app<B: Backend, T: Clone + std::fmt::Debug + std::fmt::Display>(
    terminal: &mut Terminal<B>,
    mut app: App<T>,
    tick_rate: Duration,
) -> io::Result<String> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok("You quit".to_owned()),
                    KeyCode::Left => app.items.unselect(),
                    KeyCode::Down => app.items.next(),
                    KeyCode::Up => app.items.previous(),
                    KeyCode::Enter => {
                      let result = format!("You selected: {:?}", app.items.get_selected());
                      return Ok(result)
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

fn ui<B: Backend, T: Clone + std::fmt::Debug + std::fmt::Display>(f: &mut Frame<B>, app: &mut App<T>)
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
      app.items.state.selected().and_then(|i| app.items.items.get(i).map(|x| x.to_owned()));

    // let text = vec![
    //     Spans::from(vec![
    //         Span::raw("First"),
    //         Span::styled("line",Style::default().fg(Color::Green).add_modifier(Modifier::ITALIC)),
    //         Span::raw("."),
    //     ]),
    //     Spans::from(Span::styled("Second line 🔥", Style::default().fg(Color::Red))),
    //     Spans::from(Span::styled(format!("Selected: {:?}", selected), Style::default().fg(Color::Red))),
    // ];

    let text = "blee";
    let p = Paragraph::new(text)
        .block(Block::default().title("Details").borders(Borders::ALL))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    // let banner = Text::raw("Select an item to display its details");
    f.render_widget(p, chunks[1]);
}
