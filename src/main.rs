use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    widgets::canvas::{Canvas, Points},
    Frame, Terminal,
};
use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::{self, Write},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, PartialEq)]
enum AppMode {
    Drawing,
    Selection,
    ColorSelection,
}

#[derive(Clone, Copy, PartialEq)]
enum DrawChar {
    Point,
    Horizontal,  // -
    Vertical,    // |
    Cross,       // +
    DiagRight,   // /
    DiagLeft,    // \
}

struct App {
    mode: AppMode,
    canvas: Vec<Vec<Option<DrawChar>>>,
    cursor_x: f64,
    cursor_y: f64,
    canvas_width: usize,
    canvas_height: usize,
    should_quit: bool,
    keyboard_grid: HashMap<char, (usize, usize)>,
    show_help: bool,
    current_char: DrawChar,
    current_color: Color,
    angle: f64,  // in degrees
    color_input: String,
}

impl App {
    fn new() -> App {
        let canvas_width = 80;
        let canvas_height = 40;
        let canvas = vec![vec![None; canvas_width]; canvas_height];
        
        // Create keyboard grid mapping (qwerty layout)
        let mut keyboard_grid = HashMap::new();
        let rows = [
            "qwertyuiop",
            "asdfghjkl;",
            "zxcvbnm,./",
        ];
        
        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, ch) in row.chars().enumerate() {
                keyboard_grid.insert(ch, (col_idx * 8, row_idx * 13));
            }
        }

        App {
            mode: AppMode::Drawing,
            canvas,
            cursor_x: 40.0,
            cursor_y: 20.0,
            canvas_width,
            canvas_height,
            should_quit: false,
            keyboard_grid,
            show_help: false,
            current_char: DrawChar::Point,
            current_color: Color::White,
            angle: 0.0,
            color_input: String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            AppMode::Drawing => self.handle_drawing_keys(key),
            AppMode::Selection => self.handle_selection_keys(key),
            AppMode::ColorSelection => self.handle_color_selection_keys(key),
        }
    }

    fn handle_drawing_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('h') => self.move_cursor(-1.0, 0.0),
            KeyCode::Char('j') => self.move_cursor(0.0, 1.0),
            KeyCode::Char('k') => self.move_cursor(0.0, -1.0),
            KeyCode::Char('l') => self.move_cursor(1.0, 0.0),
            KeyCode::Char('f') => self.mode = AppMode::Selection,
            KeyCode::Char(' ') => self.draw_char(),
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('c') => self.clear_canvas(),
            KeyCode::Char('s') => self.save_drawing(),
            KeyCode::Char('x') => self.mode = AppMode::ColorSelection,
            // Character selection
            KeyCode::Char('.') => self.current_char = DrawChar::Point,
            KeyCode::Char('-') => self.current_char = DrawChar::Horizontal,
            KeyCode::Char('|') => self.current_char = DrawChar::Vertical,
            KeyCode::Char('+') if key.modifiers.is_empty() => self.current_char = DrawChar::Cross,
            KeyCode::Char('/') => self.current_char = DrawChar::DiagRight,
            KeyCode::Char('\\') => self.current_char = DrawChar::DiagLeft,
            // Angle controls
            KeyCode::Char('+') if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                self.angle = (self.angle + 15.0) % 360.0;
            }
            KeyCode::Char('=') => {
                self.angle = (self.angle + 15.0) % 360.0;
            }
            KeyCode::Char('_') => {
                self.angle = (self.angle - 15.0 + 360.0) % 360.0;
            }
            _ => {}
        }
    }

    fn handle_selection_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = AppMode::Drawing,
            KeyCode::Char(ch) => {
                if let Some(&(x, y)) = self.keyboard_grid.get(&ch) {
                    self.cursor_x = (x as f64).min(self.canvas_width as f64 - 1.0);
                    self.cursor_y = (y as f64).min(self.canvas_height as f64 - 1.0);
                    self.mode = AppMode::Drawing;
                }
            }
            _ => {}
        }
    }

    fn handle_color_selection_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Drawing;
                self.color_input.clear();
            }
            KeyCode::Enter => {
                if let Some(color) = self.parse_hex_color(&self.color_input) {
                    self.current_color = color;
                }
                self.mode = AppMode::Drawing;
                self.color_input.clear();
            }
            KeyCode::Backspace => {
                self.color_input.pop();
            }
            KeyCode::Char(ch) => {
                if ch.is_ascii_hexdigit() && self.color_input.len() < 6 {
                    self.color_input.push(ch.to_ascii_uppercase());
                }
            }
            _ => {}
        }
    }

    fn move_cursor(&mut self, dx: f64, dy: f64) {
        self.cursor_x = (self.cursor_x + dx)
            .max(0.0)
            .min(self.canvas_width as f64 - 1.0);
        self.cursor_y = (self.cursor_y + dy)
            .max(0.0)
            .min(self.canvas_height as f64 - 1.0);
    }

    fn draw_char(&mut self) {
        let x = self.cursor_x as usize;
        let y = self.cursor_y as usize;
        if x < self.canvas_width && y < self.canvas_height {
            self.canvas[y][x] = Some(self.get_rotated_char());
        }
    }

    fn get_rotated_char(&self) -> DrawChar {
        if self.angle == 0.0 {
            return self.current_char;
        }
        
        // Rotate the character based on angle
        match self.current_char {
            DrawChar::Point | DrawChar::Cross => self.current_char, // These don't change with rotation
            DrawChar::Horizontal => {
                let normalized = ((self.angle + 22.5) / 45.0) as usize % 4;
                match normalized {
                    0 => DrawChar::Horizontal, // 0°
                    1 => DrawChar::DiagRight,  // 45°
                    2 => DrawChar::Vertical,   // 90°
                    3 => DrawChar::DiagLeft,   // 135°
                    _ => DrawChar::Horizontal,
                }
            }
            DrawChar::Vertical => {
                let normalized = ((self.angle + 22.5) / 45.0) as usize % 4;
                match normalized {
                    0 => DrawChar::Vertical,   // 0°
                    1 => DrawChar::DiagLeft,   // 45°
                    2 => DrawChar::Horizontal, // 90°
                    3 => DrawChar::DiagRight,  // 135°
                    _ => DrawChar::Vertical,
                }
            }
            DrawChar::DiagRight => {
                let normalized = ((self.angle + 22.5) / 45.0) as usize % 4;
                match normalized {
                    0 => DrawChar::DiagRight,  // 0°
                    1 => DrawChar::Vertical,   // 45°
                    2 => DrawChar::DiagLeft,   // 90°
                    3 => DrawChar::Horizontal, // 135°
                    _ => DrawChar::DiagRight,
                }
            }
            DrawChar::DiagLeft => {
                let normalized = ((self.angle + 22.5) / 45.0) as usize % 4;
                match normalized {
                    0 => DrawChar::DiagLeft,   // 0°
                    1 => DrawChar::Horizontal, // 45°
                    2 => DrawChar::DiagRight,  // 90°
                    3 => DrawChar::Vertical,   // 135°
                    _ => DrawChar::DiagLeft,
                }
            }
        }
    }

    fn parse_hex_color(&self, hex: &str) -> Option<Color> {
        if hex.len() != 6 {
            return None;
        }
        
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        
        Some(Color::Rgb(r, g, b))
    }

    fn save_drawing(&self) {
        if let Ok(mut file) = File::create("drawing.txt") {
            for row in &self.canvas {
                let line: String = row.iter()
                    .map(|&cell| match cell {
                        Some(DrawChar::Point) => '•',
                        Some(DrawChar::Horizontal) => '-',
                        Some(DrawChar::Vertical) => '|',
                        Some(DrawChar::Cross) => '+',
                        Some(DrawChar::DiagRight) => '/',
                        Some(DrawChar::DiagLeft) => '\\',
                        None => ' ',
                    })
                    .collect();
                let _ = writeln!(file, "{}", line.trim_end());
            }
        }
    }

    fn clear_canvas(&mut self) {
        for row in &mut self.canvas {
            for pixel in row {
                *pixel = None;
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.size());

    let canvas_widget = Canvas::default()
        .block(Block::default().title("DraVi - Vim Drawing App").borders(Borders::ALL))
        .x_bounds([0.0, app.canvas_width as f64])
        .y_bounds([0.0, app.canvas_height as f64])
        .paint(|ctx| {
            // Draw characters
            for (y, row) in app.canvas.iter().enumerate() {
                for (x, &cell) in row.iter().enumerate() {
                    if let Some(draw_char) = cell {
                        let char_to_draw = match draw_char {
                            DrawChar::Point => "•",
                            DrawChar::Horizontal => "-",
                            DrawChar::Vertical => "|",
                            DrawChar::Cross => "+",
                            DrawChar::DiagRight => "/",
                            DrawChar::DiagLeft => "\\",
                        };
                        ctx.print(
                            x as f64,
                            (app.canvas_height - 1 - y) as f64,
                            Span::styled(char_to_draw, Style::default().fg(app.current_color)),
                        );
                    }
                }
            }

            // Draw cursor
            ctx.draw(&Points {
                coords: &[(app.cursor_x, (app.canvas_height as f64 - 1.0 - app.cursor_y))],
                color: match app.mode {
                    AppMode::Drawing => Color::Red,
                    AppMode::Selection => Color::Yellow,
                    AppMode::ColorSelection => Color::Cyan,
                },
            });

            // Draw keyboard grid in selection mode
            if app.mode == AppMode::Selection {
                for (ch, &(x, y)) in &app.keyboard_grid {
                    if x < app.canvas_width && y < app.canvas_height {
                        ctx.print(
                            x as f64,
                            (app.canvas_height - 1 - y) as f64,
                            Span::styled(ch.to_string(), Style::default().fg(Color::Green)),
                        );
                    }
                }
            }
        });

    f.render_widget(canvas_widget, chunks[0]);

    let status_text = match app.mode {
        AppMode::Drawing => {
            if app.show_help {
                "hjkl:move | space:draw | -|/\\+.:chars | =/_:rotate | s:save | x:color | f:select | c:clear | ?:help | q:quit".to_string()
            } else {
                let char_name = match app.current_char {
                    DrawChar::Point => "point",
                    DrawChar::Horizontal => "horizontal",
                    DrawChar::Vertical => "vertical", 
                    DrawChar::Cross => "cross",
                    DrawChar::DiagRight => "diag-right",
                    DrawChar::DiagLeft => "diag-left",
                };
                format!("Drawing {} | angle: {}° | ? for help", char_name, app.angle)
            }
        }
        AppMode::Selection => "Selection mode - press any key to jump to that position, Esc to cancel".to_string(),
        AppMode::ColorSelection => format!("Color (hex): {} | Enter to apply, Esc to cancel", app.color_input),
    };

    let status = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[1]);
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}
