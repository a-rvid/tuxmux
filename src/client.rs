use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::time::Instant;
use tokio::sync::mpsc;
use std::time::Duration;
use std::env::args;

use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEventKind};

const CMD_NOTIFY: u8 = 0x01;
use ansi_to_tui::IntoText as _;
use ratatui::layout::{Rect, Alignment, Constraint, Layout, Position};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use ratatui::{DefaultTerminal, Frame};

#[tokio::main]
async fn main() -> Result<()> {
    let socket = args().nth(1).unwrap_or("/tmp/tuxmux.sock".to_string());
    let stream = UnixStream::connect(socket).await?;

    let (r, mut w) = stream.into_split();

    let (tx_in, rx_in) = mpsc::channel::<String>(100);
    let (tx_out, mut rx_out) = mpsc::channel::<String>(100);

    tokio::spawn(async move {
        let mut r = BufReader::new(r);
        let mut line = String::new();
        loop {
            match r.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if tx_in.send(line.clone()).await.is_err() {
                        break;
                    }
                    line.clear();
                }
                Err(_) => break,
            }
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = rx_out.recv().await {
            if w.write_all(msg.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    color_eyre::install()?;
    ratatui::run(|terminal| App::new().run(terminal, 1, rx_in, tx_out)).unwrap();
    Ok(())
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
    notifications: Vec<Notification>,
    welcome_content: Option<String>,
}

struct Notification {
    message: String,
    time: Instant,
}

enum InputMode {
    Normal,
    Command,
    Insert,
}

enum ServerMsg {
    /// Prefixed with '1' — the server MOTD, sent once on connect
    Motd(String),
    /// Prefixed with '2' — transient notification
    Notification(String),
    /// No prefix — regular shell output
    Output(String),
}

impl ServerMsg {
    fn parse(line: String) -> Self {
        match line.chars().next() {
            Some('1') => Self::Motd(line[1..].trim_end_matches('\n').replace("\\n", "\n")),
            Some('2') => Self::Notification(line[1..].trim_end_matches('\n').to_string()),
            _ => Self::Output(line.trim_end_matches('\n').to_string()),
        }
    }
}

enum Command {
    Quit,
    Help,
    All(String),
    Notify(String),
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
                "n" | "notify" => Self::Notify(rest.to_string()),
                _ => Self::Unknown(input.to_string()),
            },
            // no space => command only
            None => match input {
                "q" | "quit" => Self::Quit,
                "h" | "help" => Self::Help,
                "n" | "notify" => Self::Notify(String::new()),
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
            notifications: Vec::new(),
            welcome_content: None,
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

    fn cmd(&mut self) -> Command {
        let cmd = Command::parse(&self.input);
        self.input.clear();
        self.reset_cursor();
        cmd
    }

    fn run(&mut self, terminal: &mut DefaultTerminal, shell: usize, mut rx: mpsc::Receiver<String>, tx: mpsc::Sender<String>) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame, shell))?;

            // Remove notifications that have been displayed for more than 5 seconds
            self.notifications.retain(|n| n.time.elapsed() < Duration::from_secs(5));

            while let Ok(line) = rx.try_recv() {
                match ServerMsg::parse(line) {
                    ServerMsg::Motd(content) => {
                        self.welcome_content = Some(content);
                    }
                    ServerMsg::Notification(msg) => {
                        self.notifications.push(Notification {
                            message: msg,
                            time: Instant::now(),
                        });
                    }
                    ServerMsg::Output(msg) => {
                        self.messages.push(msg);
                    }
                }
            }

            if event::poll(Duration::from_millis(10))? {
                if let Some(key) = event::read()?.as_key_press_event() {
                    match self.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char(':') => {
                                self.input_mode = InputMode::Command;
                                self.show_help = false;
                                self.show_welcome = false;
                            }
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
                                    let _ = tx.blocking_send(format!("{}\n", arg));
                                    self.show_help = false;
                                    self.input_mode = InputMode::Normal;
                                },
                                Command::Notify(msg) => {
                                    let tx_clone = tx.clone();
                                    let msg_to_send = format!("{}{}\n", CMD_NOTIFY as char, msg);
                                    tokio::spawn(async move {
                                        let _ = tx_clone.send(msg_to_send).await;
                                    });
                                    self.show_help = false;
                                    self.input_mode = InputMode::Normal;
                                },
                                Command::Unknown(_input) => {
                                    self.input_mode = InputMode::Normal;
                                }
                            },
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Esc => self.input_mode = InputMode::Normal,
                            _ => {}
                        },
                        InputMode::Insert if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Enter => {
                                let _ = tx.blocking_send(format!("{}\n", self.input));
                                self.input.clear();
                                self.reset_cursor();
                            }
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Esc => self.input_mode = InputMode::Normal,
                            _ => {}
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame, shell: usize) {
        let layout = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1)
        ]);
        let [messages_area, help_area, input_area] = frame.area().layout(&layout);

        match self.input_mode {
            InputMode::Normal => {}
            #[expect(clippy::cast_possible_truncation)]
            InputMode::Command => frame.set_cursor_position(Position::new(
                input_area.x + self.character_index as u16 + 1,
                input_area.y + 1,
            )),
            InputMode::Insert => {} 
        }
        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.lines().enumerate().map(move |(j, line)| {
                    let content = Line::from(Span::raw(format!("{i}.{j}: {line}")));
                    ListItem::new(content)
                })
            })
            .collect();
        let messages = List::new(messages).block(Block::bordered().title(format!("Client {}", shell)));
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

        if !self.notifications.is_empty() {
            let width = 30;
            let height = (self.notifications.len() as u16).saturating_add(2);
            let area = top_right_rect(width, height, frame.area());

            let notifications: Vec<ListItem> = self
                .notifications
                .iter()
                .map(|notification| {
                    let content = Line::from(vec![
                        " ! ".bold().yellow(),
                        notification.message.clone().into(),
                    ]);
                    ListItem::new(content)
                })
                .collect();
            let notifications = List::new(notifications)
                .block(
                    Block::bordered()
                        .title(" Notifications ")
                        .title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .style(Style::default().bg(Color::Black));
            frame.render_widget(notifications, area);
        }

        if self.show_welcome {
            let area = centered_rect(60, 40, frame.area());
            let content = if let Some(content) = &self.welcome_content {
                content.clone().into_text().unwrap()
            } else {
                const ENTER: &str = "\x1b[90m<ENTER>\x1b[0m";
                format!(
                    "Welcome to TuxMux!\n
https://github.com/a-rvid/tuxmux/\n
\n
type  :h | :help{ENTER}      if you are new        
type  :q | :quit{ENTER}      to exit               
type  :a | :all{ENTER}       to send to all clients
type  i{ENTER}               to enter insert mode  
type  Escape{ENTER}          to enter normal mode  
"
                )
                .into_text()
                .unwrap()
            };
            let welcome = Paragraph::new(content).alignment(Alignment::Center);
            frame.render_widget(welcome, area);
        }
    }
}

fn top_right_rect(width: u16, height: u16, area: Rect) -> Rect {
    let top_row = Layout::vertical([
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .split(area)[0];

    Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(width),
    ])
    .split(top_row)[1]
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