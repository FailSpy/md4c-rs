//! Basic demo of ratatui-md rendering.
//!
//! Run with: cargo run --example demo

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders},
    Terminal,
};
use ratatui_md::{Markdown, Theme};
use std::io;

const DEMO_MARKDOWN: &str = r#"# ratatui-md Demo

Welcome to the **ratatui-md** demo! This showcases various Markdown features.

## Text Formatting

Here's some *italic text*, **bold text**, and ***bold italic***.
You can also use ~~strikethrough~~ text.

## Links

Check out [ratatui](https://github.com/ratatui-org/ratatui) for building TUIs!
And [MD4C](https://github.com/mity/md4c) for fast Markdown parsing.

## Lists

### Unordered List
- First item
- Second item
  - Nested item
  - Another nested
- Third item

### Ordered List
1. Step one
2. Step two
3. Step three

### Task List
- [x] Completed task
- [ ] Pending task
- [x] Another done

## Code

Inline `code` looks like this.

```rust
fn main() {
    println!("Hello, ratatui!");
}
```

## Blockquote

> This is a blockquote.
> It can span multiple lines.
>
> And have multiple paragraphs.

## Tables

| Feature | Status | Notes |
|---------|:------:|------:|
| Headings | ✓ | All levels |
| Emphasis | ✓ | Bold, italic |
| Lists | ✓ | Ordered, unordered |
| Tables | ✓ | With alignment |

---

## Horizontal Rule

The line above is a horizontal rule.

## Conclusion

Press **q** to quit this demo.
"#;

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal);

    // Restore terminal
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
    let mut scroll: u16 = 0;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();

            let markdown = Markdown::new(DEMO_MARKDOWN)
                .theme(Theme::dark())
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title(" ratatui-md Demo (q=quit, j/k=scroll) ")
                        .borders(Borders::ALL),
                );

            frame.render_widget(markdown, area);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => scroll = scroll.saturating_add(1),
                    KeyCode::Char('k') | KeyCode::Up => scroll = scroll.saturating_sub(1),
                    KeyCode::PageDown => scroll = scroll.saturating_add(10),
                    KeyCode::PageUp => scroll = scroll.saturating_sub(10),
                    KeyCode::Home => scroll = 0,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
