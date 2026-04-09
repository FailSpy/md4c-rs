//! Interactive scrollable markdown viewer with navigation.
//!
//! Run with: cargo run --example scrollable
//!
//! Features:
//! - Scrolling with j/k or arrow keys
//! - Jump to headings with number keys
//! - Navigate links with Tab/Shift+Tab
//! - Show table of contents with 't'

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use ratatui_md::{MarkdownView, Theme};
use std::io;

const SAMPLE_DOC: &str = r##"# Markdown Viewer

This is an interactive markdown document viewer built with **ratatui-md**.

## Navigation

Use the following keys to navigate:

- `j` / `↓` - Scroll down
- `k` / `↑` - Scroll up
- `g` - Go to top
- `G` - Go to bottom
- `t` - Toggle table of contents
- `Tab` - Next link
- `Shift+Tab` - Previous link
- `1-9` - Jump to heading
- `q` - Quit

## Features

### Rich Text

This viewer supports all standard Markdown formatting:
*italic*, **bold**, `inline code`, and ~~strikethrough~~.

### Links

Here are some useful links:
- [Rust Programming Language](https://rust-lang.org)
- [ratatui Documentation](https://docs.rs/ratatui)
- [MD4C GitHub](https://github.com/mity/md4c)

### Code Blocks

```rust
use ratatui_md::MarkdownView;

fn main() {
    let mut view = MarkdownView::new("# Hello World");
    view.scroll_down(10);
}
```

### Tables

| Command | Description |
|---------|-------------|
| `j`/`k` | Scroll up/down |
| `g`/`G` | Top/Bottom |
| `t` | Toggle TOC |
| `q` | Quit |

### Lists

1. First ordered item
2. Second ordered item
   - Nested unordered
   - Another nested
3. Third ordered item

Task list:
- [x] Implement scrolling
- [x] Add heading navigation
- [x] Support tables
- [ ] Add search

### Blockquotes

> "The best way to predict the future is to invent it."
> — Alan Kay

## About

This example demonstrates the full capabilities of **ratatui-md** for building
documentation viewers, help systems, and other text-heavy terminal applications.

---

*Press `q` to exit.*
"##;

struct App {
    view: MarkdownView,
    show_toc: bool,
    viewport_height: u16,
}

impl App {
    fn new() -> Self {
        Self {
            view: MarkdownView::new(SAMPLE_DOC).theme(Theme::dark()),
            show_toc: false,
            viewport_height: 20,
        }
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            app.viewport_height = area.height.saturating_sub(2);

            // Main content area
            let text = app.view.text().clone();
            let scroll = app.view.scroll_offset();
            let line_count = app.view.line_count();

            // Build status line
            let selected_link = app.view.selected_link().map(|l| l.url.clone());
            let status = if let Some(url) = selected_link {
                format!(" Link: {} ", url)
            } else {
                format!(
                    " Line {}/{} | Press 't' for TOC, 'q' to quit ",
                    scroll + 1,
                    line_count
                )
            };

            let main_block = Block::default()
                .title(" Markdown Viewer ")
                .title_bottom(Line::from(status).centered())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(text)
                .block(main_block)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0));

            frame.render_widget(paragraph, area);

            // Table of contents overlay
            if app.show_toc {
                let headings = app.view.headings();
                let toc_items: Vec<ListItem> = headings
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        let indent = "  ".repeat((h.level - 1) as usize);
                        let prefix = if i < 9 {
                            format!("[{}] ", i + 1)
                        } else {
                            "    ".to_string()
                        };
                        ListItem::new(format!("{}{}{}", prefix, indent, h.text))
                    })
                    .collect();

                let toc_width = 40.min(area.width.saturating_sub(4));
                let toc_height = (headings.len() as u16 + 2).min(area.height.saturating_sub(4));
                let toc_area = centered_rect(toc_width, toc_height, area);

                let toc_block = Block::default()
                    .title(" Table of Contents ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .style(Style::default().bg(Color::Black));

                let toc = List::new(toc_items).block(toc_block);

                frame.render_widget(Clear, toc_area);
                frame.render_widget(toc, toc_area);
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => app.view.scroll_down(1),
                    KeyCode::Char('k') | KeyCode::Up => app.view.scroll_up(1),
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.view.scroll_down(app.viewport_height / 2);
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.view.scroll_up(app.viewport_height / 2);
                    }
                    KeyCode::PageDown => app.view.scroll_down(app.viewport_height),
                    KeyCode::PageUp => app.view.scroll_up(app.viewport_height),
                    KeyCode::Char('g') => app.view.scroll_to_top(),
                    KeyCode::Char('G') => app.view.scroll_to_bottom(),
                    KeyCode::Char('t') => app.show_toc = !app.show_toc,
                    KeyCode::Tab => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            app.view.select_prev_link();
                        } else {
                            app.view.select_next_link();
                        }
                    }
                    KeyCode::BackTab => app.view.select_prev_link(),
                    KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                        let idx = c.to_digit(10).unwrap() as usize - 1;
                        app.view.scroll_to_heading(idx);
                        app.show_toc = false;
                    }
                    KeyCode::Esc => app.show_toc = false,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
