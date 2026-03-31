use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::{self, Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ansi_to_tui::IntoText as _;
use ratatui::layout::{Rect, Alignment, Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

#[tokio::main]
async fn main() -> Result<()> {
    let stream = UnixStream::connect("/tmp/tuxmux.sock").await?;
    let (mut r, mut w) = stream.into_split();

    // w.write_all to execute
    // socket -> stdout
    // let tx = Arc::new(Mutex::new(w));

    let mut stdout = io::stdout();
    let mut buf = [0u8; 1024];
    tokio::spawn(async move {
        loop {
            let n = r.read(&mut buf).await.unwrap();
            if n == 0 { break; }
            let s = String::from_utf8_lossy(&buf[..n]);
        }
    });

    color_eyre::install()?;
    ratatui::run(|terminal| App::new().run(terminal))
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: String,
    /// Position of cursor in the editor area.
    character_index: usize,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Vec<String>,
    /// Show help message
    show_help: bool,
    show_welcome: bool,
}

enum InputMode {
    Normal,
    Command,
    Insert,
}

enum Command {
    Quit,
    Help,
    All(String),
    Unknown(String),
}

impl Command {
    fn parse(input: &str) -> Self {
        match input.split_once(' ') {
            // first and rest here are &str from split_once
            Some((first, rest)) => match first {
                "q" | "quit" => Self::Quit,
                "h" | "help" => Self::Help,
                "a" | "all" => Self::All(rest.to_string()),
                _ => Self::Unknown(input.to_string()),
            },
            // no space => command only
            None => match input {
                "q" | "quit" => Self::Quit,
                "h" | "help" => Self::Help,
                _ => Self::Unknown(input.to_string()),
            },
        }
    }
}

impl App {
    const fn new() -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            character_index: 0,
            show_help: false,
            show_welcome: true,
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    const fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    // fn submit_message(&mut self) {
    //     self.messages.push(self.input.clone());
    //     self.input.clear();
    //     self.reset_cursor();
    // }

    fn cmd(&mut self) -> Command {
        let cmd = Command::parse(&self.input);
        self.input.clear();
        self.reset_cursor();
        return cmd
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if let Some(key) = event::read()?.as_key_press_event() {
                match self.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char(':') => {
                            self.input_mode = InputMode::Command;
                            self.show_help = false;
                            self.show_welcome = false;
                        }
                        // KeyCode::Char('q') => {
                        //     return Ok(());
                        // }
                        KeyCode::Char('i') => {
                            self.input_mode = InputMode::Insert;
                            self.show_help = false;
                            self.show_welcome = false;
                        }
                        _ => {
                            self.show_help = false;
                            self.show_welcome = false;
                        }
                    },
                    InputMode::Command if key.kind == KeyEventKind::Press => match key.code {
                        KeyCode::Enter => match self.cmd() {
                            Command::Quit => if self.show_help { self.show_help = false; } else { return Ok(()); },
                            Command::Help => { self.show_help = true; self.input_mode = InputMode::Normal; },
                            Command::All(arg) => {
                                // tx.write_all(arg.as_bytes()).await.unwrap();
                                self.show_help = false;
                                self.input_mode = InputMode::Normal; 
                            },
                            _cmd => self.input_mode = InputMode::Normal,
                        },
                        KeyCode::Char(to_insert) => self.enter_char(to_insert),
                        KeyCode::Backspace => self.delete_char(),
                        KeyCode::Left => self.move_cursor_left(),
                        KeyCode::Right => self.move_cursor_right(),
                        KeyCode::Esc => self.input_mode = InputMode::Normal,
                        _ => {}
                    },
                    InputMode::Command => {}
                    InputMode::Insert => {}
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]);
        let [messages_area, help_area, input_area] = frame.area().layout(&layout);

        match self.input_mode {
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            InputMode::Normal => {}

            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[expect(clippy::cast_possible_truncation)]
            InputMode::Command => frame.set_cursor_position(Position::new(
                // Draw the cursor at the current position in the input field.
                // This position can be controlled via the left and right arrow key
                input_area.x + self.character_index as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )),

            InputMode::Insert => {} 
        }

        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let content = Line::from(Span::raw(format!("{i}: {m}")));
                ListItem::new(content)
            })
            .collect();
        let messages = List::new(messages).block(Block::bordered().title("Client"));
        frame.render_widget(messages, messages_area);

        let (msg, style) = match self.input_mode {
            InputMode::Normal => (
                vec![
                    "Normal".bold().bg(Color::Green),
                ],
                Style::default().add_modifier(Modifier::RAPID_BLINK),
            ),
            InputMode::Command => (
                vec![
                    "Command".bold().bg(Color::Yellow),
                ],
                Style::default(),
            ),
            InputMode::Insert => (
                vec![
                    "Insert".bold().bg(Color::Blue),
                ],
                Style::default(),
            ),
        };

        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let content = match self.input_mode {
            InputMode::Command => format!(":{}", self.input),
                _ => self.input.clone(),
            };

        let input = Paragraph::new(content)
            .style(match self.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Command => Style::default().add_modifier(Modifier::RAPID_BLINK),
                InputMode::Insert => Style::default().fg(Color::Yellow),
            });

        frame.render_widget(input, input_area);

        if self.show_help {
            let area = centered_rect(60, 40, frame.area());

            let help = Paragraph::new(
                ":h | :help - Show this help message\n:q | :quit - Quit TuxMux (or close overlay)\ni - Insert mode\n\nPress Esc to switch to normal mode.\n",
            )
            .block(Block::bordered().title("Help"))
            .style(Style::default().bg(Color::Black));

            frame.render_widget(help, area);
        }
        
        const ENTER: &str = "\x1b[90m<ENTER>\x1b[0m";
        if self.show_welcome {
            let area = centered_rect(60, 40, frame.area());
            let line = format!("Welcome to TuxMux!\n
https://github.com/a-rvid/tuxmux/\n
\n
type  :h | :help{ENTER}      if you are new      
type  :q | :quit{ENTER}      to exit             
type  :a | :all{ENTER}       to send to all clients
type  i{ENTER}               to enter insert mode
type  Escape{ENTER}          to enter normal mode
").into_text().unwrap();
            let welcome = Paragraph::new(line).alignment(Alignment::Center);
            frame.render_widget(welcome, area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ]);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ]);

    let [_, middle, _] = vertical.areas(area);
    let [_, center, _] = horizontal.areas(middle);
    center
}