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
    widgets::canvas::{Canvas, Points, Line},
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
    CoordinateInput,
    TypstInput,
    Settings,
    PdfRender,
}

#[derive(Clone, Copy, PartialEq)]
enum CoordinateSystem {
    Cartesian,
    Polar,
    Cylindrical,
}

#[derive(Clone, PartialEq)]
enum DrawChar {
    Point,
    Horizontal,  // -
    Vertical,    // |
    Cross,       // +
    DiagRight,   // /
    DiagLeft,    // \
    Text(char),  // Any ASCII character
}

struct App {
    mode: AppMode,
    canvas: Vec<Vec<Option<DrawChar>>>,
    cursor_x: f64,
    cursor_y: f64,
    canvas_width: usize,
    canvas_height: usize,
    virtual_height: usize,
    scroll_y: usize,
    should_quit: bool,
    keyboard_grid: HashMap<char, (usize, usize)>,
    current_char: DrawChar,
    current_color: Color,
    color_input: String,
    continuous_draw: bool,
    last_cursor_x: f64,
    last_cursor_y: f64,
    coordinate_system: CoordinateSystem,
    show_axes: bool,
    coordinate_input: String,
    origin_x: f64,
    origin_y: f64,
    grid_snap: bool,
    text_buffer: String,
}

impl App {
    fn new() -> App {
        let canvas_width = 80;
        let canvas_height = 40;
        let virtual_height = 200; // Allow scrolling to 200 lines
        let canvas = vec![vec![None; canvas_width]; virtual_height];
        
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
            virtual_height,
            scroll_y: 0,
            should_quit: false,
            keyboard_grid,
            current_char: DrawChar::Point,
            current_color: Color::Rgb(255, 105, 180), // Hot pink
            color_input: String::new(),
            continuous_draw: false,
            last_cursor_x: 40.0,
            last_cursor_y: 20.0,
            coordinate_system: CoordinateSystem::Cartesian,
            show_axes: true,
            coordinate_input: String::new(),
            origin_x: 40.0,
            origin_y: 20.0,
            grid_snap: false,
            text_buffer: String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            AppMode::Drawing => self.handle_drawing_keys(key),
            AppMode::Selection => self.handle_selection_keys(key),
            AppMode::ColorSelection => self.handle_color_selection_keys(key),
            AppMode::CoordinateInput => self.handle_coordinate_input_keys(key),
            AppMode::TypstInput => self.handle_typst_input_keys(key),
            AppMode::Settings => self.handle_settings_keys(key),
            AppMode::PdfRender => self.handle_pdf_render_keys(key),
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
            KeyCode::Char('?') => self.mode = AppMode::Settings,
            KeyCode::Char('c') => self.clear_canvas(),
            KeyCode::Char('s') => self.save_typst(),
            KeyCode::Char('x') => self.mode = AppMode::ColorSelection,
            KeyCode::Char('d') => self.continuous_draw = !self.continuous_draw,
            KeyCode::Char('a') => self.show_axes = !self.show_axes,
            KeyCode::Char('g') => self.mode = AppMode::CoordinateInput,
            KeyCode::Char('i') => self.mode = AppMode::TypstInput,
            KeyCode::Char('n') => self.grid_snap = !self.grid_snap,
            // Character selection
            KeyCode::Char('.') => self.current_char = DrawChar::Point,
            KeyCode::Char('-') => self.current_char = DrawChar::Horizontal,
            KeyCode::Char('|') => self.current_char = DrawChar::Vertical,
            KeyCode::Char('+') => self.current_char = DrawChar::Cross,
            KeyCode::Char('/') => self.current_char = DrawChar::DiagRight,
            KeyCode::Char('\\') => self.current_char = DrawChar::DiagLeft,
            // Coordinate system switching
            KeyCode::Char('1') => self.coordinate_system = CoordinateSystem::Cartesian,
            KeyCode::Char('2') => self.coordinate_system = CoordinateSystem::Polar,
            KeyCode::Char('3') => self.coordinate_system = CoordinateSystem::Cylindrical,
            // Origin setting
            KeyCode::Char('o') => {
                self.origin_x = self.cursor_x;
                self.origin_y = self.cursor_y;
            }
            // Scrolling
            KeyCode::Char('J') => self.scroll_down(),
            KeyCode::Char('K') => self.scroll_up(),
            KeyCode::Char('r') => {
                self.open_pdf();
                self.mode = AppMode::PdfRender;
            },
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

    fn handle_coordinate_input_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Drawing;
                self.coordinate_input.clear();
            }
            KeyCode::Enter => {
                self.parse_and_move_to_coordinate();
                self.mode = AppMode::Drawing;
                self.coordinate_input.clear();
            }
            KeyCode::Backspace => {
                self.coordinate_input.pop();
            }
            KeyCode::Char(ch) => {
                if (ch.is_ascii_digit() || ch == '.' || ch == ',' || ch == ' ' || ch == '-') 
                   && self.coordinate_input.len() < 20 {
                    self.coordinate_input.push(ch);
                }
            }
            _ => {}
        }
    }

    fn parse_and_move_to_coordinate(&mut self) {
        let parts: Vec<&str> = self.coordinate_input.split(',').collect();
        
        match self.coordinate_system {
            CoordinateSystem::Cartesian => {
                if parts.len() >= 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                        self.cursor_x = (self.origin_x + x).clamp(0.0, self.canvas_width as f64 - 1.0);
                        self.cursor_y = (self.origin_y - y).clamp(0.0, self.canvas_height as f64 - 1.0); // Flip Y for screen coords
                    }
                }
            }
            CoordinateSystem::Polar => {
                if parts.len() >= 2 {
                    if let (Ok(r), Ok(theta)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                        let x = r * theta.to_radians().cos();
                        let y = r * theta.to_radians().sin();
                        self.cursor_x = (self.origin_x + x).clamp(0.0, self.canvas_width as f64 - 1.0);
                        self.cursor_y = (self.origin_y - y).clamp(0.0, self.canvas_height as f64 - 1.0);
                    }
                }
            }
            CoordinateSystem::Cylindrical => {
                if parts.len() >= 3 {
                    if let (Ok(r), Ok(theta), Ok(z)) = (
                        parts[0].trim().parse::<f64>(), 
                        parts[1].trim().parse::<f64>(), 
                        parts[2].trim().parse::<f64>()
                    ) {
                        let x = r * theta.to_radians().cos();
                        let y = r * theta.to_radians().sin() + z * 0.1; // Simple z representation
                        self.cursor_x = (self.origin_x + x).clamp(0.0, self.canvas_width as f64 - 1.0);
                        self.cursor_y = (self.origin_y - y).clamp(0.0, self.canvas_height as f64 - 1.0);
                    }
                }
            }
        }
    }

    fn handle_typst_input_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Drawing;
                self.text_buffer.clear();
            }
            KeyCode::Enter => {
                // Place the text buffer on canvas and move to next line
                for (i, ch) in self.text_buffer.chars().enumerate() {
                    let x = (self.cursor_x as usize + i).min(self.canvas_width - 1);
                    let y = self.cursor_y as usize;
                    if x < self.canvas_width && y < self.virtual_height {
                        self.canvas[y][x] = Some(DrawChar::Text(ch));
                    }
                }
                self.move_cursor(0.0, 1.0); // New line
                self.cursor_x = self.origin_x; // Reset to left margin
                self.text_buffer.clear();
                self.mode = AppMode::Drawing; // Return to drawing mode
            }
            KeyCode::Backspace => {
                if !self.text_buffer.is_empty() {
                    self.text_buffer.pop();
                } else {
                    // Move cursor back and delete character
                    self.move_cursor(-1.0, 0.0);
                    let x = self.cursor_x as usize;
                    let y = self.cursor_y as usize;
                    if x < self.canvas_width && y < self.virtual_height {
                        self.canvas[y][x] = None;
                    }
                }
            }
            KeyCode::Char(ch) => {
                if ch != '\0' && !ch.is_control() {
                    self.text_buffer.push(ch);
                    
                    // Auto-completion for paired characters
                    match ch {
                        '(' => self.text_buffer.push(')'),
                        '[' => self.text_buffer.push(']'),
                        '{' => self.text_buffer.push('}'),
                        '$' => self.text_buffer.push('$'),
                        '"' => self.text_buffer.push('"'),
                        '\'' => self.text_buffer.push('\''),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_settings_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => self.mode = AppMode::Drawing,
            KeyCode::Char('a') => self.show_axes = !self.show_axes,
            KeyCode::Char('n') => self.grid_snap = !self.grid_snap,
            KeyCode::Char('d') => self.continuous_draw = !self.continuous_draw,
            KeyCode::Char('1') => self.coordinate_system = CoordinateSystem::Cartesian,
            KeyCode::Char('2') => self.coordinate_system = CoordinateSystem::Polar,
            KeyCode::Char('3') => self.coordinate_system = CoordinateSystem::Cylindrical,
            _ => {}
        }
    }
    
    fn handle_pdf_render_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('r') => self.mode = AppMode::Drawing,
            _ => {}
        }
    }
    
    fn open_pdf(&self) {
        use std::process::Command;
        
        // Open PDF with tdf in a new terminal
        let terminals = [("alacritty", vec!["-e", "tdf", "drawing.pdf"]),
                        ("gnome-terminal", vec!["--", "tdf", "drawing.pdf"]),
                        ("xterm", vec!["-e", "tdf", "drawing.pdf"]),
                        ("konsole", vec!["-e", "tdf", "drawing.pdf"])];
        
        for (terminal, args) in &terminals {
            if let Ok(_) = Command::new(terminal)
                .args(args)
                .spawn() {
                break;
            }
        }
    }
    
    fn scroll_up(&mut self) {
        self.scroll_y = self.scroll_y.saturating_sub(3);
    }
    
    fn scroll_down(&mut self) {
        self.scroll_y = (self.scroll_y + 3).min(self.virtual_height.saturating_sub(self.canvas_height));
    }

    fn get_current_coordinates(&self) -> String {
        let rel_x = self.cursor_x - self.origin_x;
        let rel_y = self.origin_y - self.cursor_y; // Flip Y for mathematical coords

        match self.coordinate_system {
            CoordinateSystem::Cartesian => {
                format!("({:.1}, {:.1})", rel_x, rel_y)
            }
            CoordinateSystem::Polar => {
                let r = (rel_x * rel_x + rel_y * rel_y).sqrt();
                let theta = rel_y.atan2(rel_x).to_degrees();
                format!("(r:{:.1}, θ:{:.1}°)", r, theta)
            }
            CoordinateSystem::Cylindrical => {
                let r = (rel_x * rel_x).sqrt();
                let theta = rel_y.atan2(rel_x).to_degrees();
                let z = rel_y * 10.0; // Simple z representation
                format!("(ρ:{:.1}, θ:{:.1}°, z:{:.1})", r, theta, z)
            }
        }
    }

    fn move_cursor(&mut self, dx: f64, dy: f64) {
        self.last_cursor_x = self.cursor_x;
        self.last_cursor_y = self.cursor_y;
        
        let mut new_x = self.cursor_x + dx;
        let mut new_y = self.cursor_y + dy;
        
        // Grid snapping
        if self.grid_snap {
            new_x = new_x.round();
            new_y = new_y.round();
        }
        
        self.cursor_x = new_x.max(0.0).min(self.canvas_width as f64 - 1.0);
        self.cursor_y = new_y.max(0.0).min(self.virtual_height as f64 - 1.0);
        
        // Auto-scroll to follow cursor
        let visible_start = self.scroll_y;
        let visible_end = self.scroll_y + self.canvas_height - 1;
        
        if (self.cursor_y as usize) < visible_start {
            self.scroll_y = (self.cursor_y as usize).max(0);
        } else if (self.cursor_y as usize) > visible_end {
            self.scroll_y = ((self.cursor_y as usize) + 1).saturating_sub(self.canvas_height).min(self.virtual_height - self.canvas_height);
        }
            
        if self.continuous_draw {
            self.draw_line_to_cursor();
        }
    }

    fn draw_line_to_cursor(&mut self) {
        let x0 = self.last_cursor_x as i32;
        let y0 = self.last_cursor_y as i32;
        let x1 = self.cursor_x as i32;
        let y1 = self.cursor_y as i32;
        
        // Bresenham's line algorithm
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            if x >= 0 && x < self.canvas_width as i32 && y >= 0 && y < self.virtual_height as i32 {
                self.canvas[y as usize][x as usize] = Some(self.current_char.clone());
            }
            
            if x == x1 && y == y1 { break; }
            
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn draw_char(&mut self) {
        let x = self.cursor_x as usize;
        let y = self.cursor_y as usize;
        if x < self.canvas_width && y < self.virtual_height {
            self.canvas[y][x] = Some(self.current_char.clone());
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


    fn save_typst(&self) {
        if let Ok(mut file) = File::create("drawing.typ") {
            let _ = writeln!(file, "#set page(margin: 0.5in, fill: black)");
            let _ = writeln!(file, "#set text(size: 12pt, fill: rgb(\"#ff69b4\"))");
            let _ = writeln!(file, "#set par(leading: 0.6em)");
            let _ = writeln!(file, "");
            let _ = writeln!(file, "= Mathematical Calculations");
            let _ = writeln!(file, "");
            
            // Check if we have text content
            let has_text = self.canvas.iter().any(|row| {
                row.iter().any(|cell| matches!(cell, Some(DrawChar::Text(_))))
            });
            
            if has_text {
                // Collect text content and render as normal Typst paragraphs
                let mut text_lines = Vec::new();
                
                for row in &self.canvas {
                    let mut has_text_content = false;
                    let mut text_content = String::new();
                    
                    for cell in row {
                        match cell {
                            Some(DrawChar::Text(ch)) => {
                                text_content.push(*ch);
                                has_text_content = true;
                            },
                            Some(DrawChar::Point) => text_content.push('•'),
                            Some(DrawChar::Horizontal) => text_content.push('-'),
                            Some(DrawChar::Vertical) => text_content.push('|'),
                            Some(DrawChar::Cross) => text_content.push('+'),
                            Some(DrawChar::DiagRight) => text_content.push('/'),
                            Some(DrawChar::DiagLeft) => text_content.push('\\'),
                            None => {
                                if has_text_content {
                                    text_content.push(' ');
                                }
                            },
                        }
                    }
                    
                    if has_text_content {
                        text_lines.push(text_content.trim_end().to_string());
                    }
                }
                
                // Join lines into natural paragraphs
                let mut paragraph = String::new();
                for line in text_lines {
                    if line.trim().is_empty() {
                        if !paragraph.is_empty() {
                            // Output current paragraph
                            if paragraph.contains('$') {
                                let _ = writeln!(file, "{}", paragraph.trim());
                            } else if paragraph.matches('=').count() == 1 && 
                                      (paragraph.contains('+') || paragraph.contains('-') || 
                                       paragraph.contains('*') || paragraph.contains('/')) {
                                let _ = writeln!(file, "${}", paragraph.trim());
                            } else {
                                let _ = writeln!(file, "{}", paragraph.trim());
                            }
                            let _ = writeln!(file, "");
                            paragraph.clear();
                        }
                    } else {
                        if !paragraph.is_empty() {
                            paragraph.push(' ');
                        }
                        paragraph.push_str(&line);
                    }
                }
                
                // Output final paragraph
                if !paragraph.is_empty() {
                    if paragraph.contains('$') {
                        let _ = writeln!(file, "{}", paragraph.trim());
                    } else if paragraph.matches('=').count() == 1 && 
                              (paragraph.contains('+') || paragraph.contains('-') || 
                               paragraph.contains('*') || paragraph.contains('/')) {
                        let _ = writeln!(file, "${}", paragraph.trim());
                    } else {
                        let _ = writeln!(file, "{}", paragraph.trim());
                    }
                }
            } else {
                // Pure ASCII art drawing
                let _ = writeln!(file, "```");
                for row in &self.canvas {
                    let line: String = row.iter()
                        .map(|cell| match cell {
                            Some(DrawChar::Point) => '•',
                            Some(DrawChar::Horizontal) => '-',
                            Some(DrawChar::Vertical) => '|',
                            Some(DrawChar::Cross) => '+',
                            Some(DrawChar::DiagRight) => '/',
                            Some(DrawChar::DiagLeft) => '\\',
                            Some(DrawChar::Text(ch)) => *ch,
                            None => ' ',
                        })
                        .collect();
                    let _ = writeln!(file, "{}", line.trim_end());
                }
                let _ = writeln!(file, "```");
            }
            
        }
        
        // Auto-compile to PDF if typst is available
        self.compile_to_pdf();
    }
    
    fn compile_to_pdf(&self) {
        use std::process::Command;
        
        // Try to compile with typst
        match Command::new("typst")
            .args(["compile", "drawing.typ"])
            .output() {
            Ok(output) => {
                if output.status.success() {
                    // Success - PDF created
                } else {
                    // Failed - but don't interrupt the user
                }
            }
            Err(_) => {
                // typst command not found - ignore silently
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
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.size());
        
    let chunks = if app.mode == AppMode::Settings {
        // Split main area for settings popup
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(30)].as_ref())
            .split(main_chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0)].as_ref())
            .split(main_chunks[0])
    };

    let canvas_widget = Canvas::default()
        .block(Block::default()
            .title("DraVi - Mathematical Drawing Tool")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Rgb(100, 149, 237)).bg(Color::Black)))
        .x_bounds([0.0, app.canvas_width as f64])
        .y_bounds([0.0, app.canvas_height as f64])
        .background_color(Color::Black)
        .paint(|ctx| {
            
            // Draw axes if enabled
            if app.show_axes {
                // X-axis (horizontal line through origin)
                ctx.draw(&Line {
                    x1: 0.0,
                    y1: app.canvas_height as f64 - 1.0 - app.origin_y,
                    x2: app.canvas_width as f64 - 1.0,
                    y2: app.canvas_height as f64 - 1.0 - app.origin_y,
                    color: Color::Red,
                });
                
                // Y-axis (vertical line through origin)
                ctx.draw(&Line {
                    x1: app.origin_x,
                    y1: 0.0,
                    x2: app.origin_x,
                    y2: app.canvas_height as f64 - 1.0,
                    color: Color::Red,
                });

                // Origin marker
                ctx.draw(&Points {
                    coords: &[(app.origin_x, app.canvas_height as f64 - 1.0 - app.origin_y)],
                    color: Color::Red,
                });
            }

            // Draw characters (only visible portion)
            for (y, row) in app.canvas.iter().enumerate().skip(app.scroll_y).take(app.canvas_height) {
                for (x, cell) in row.iter().enumerate() {
                    if let Some(draw_char) = cell {
                        match draw_char {
                            DrawChar::Text(ch) => {
                                ctx.print(
                                    x as f64,
                                    (app.canvas_height as f64 - 1.0 - ((y - app.scroll_y) as f64)),
                                    Span::styled(ch.to_string(), Style::default().fg(app.current_color)),
                                );
                            }
                            _ => {
                                let char_to_draw = match draw_char {
                                    DrawChar::Point => "•",
                                    DrawChar::Horizontal => "-",
                                    DrawChar::Vertical => "|",
                                    DrawChar::Cross => "+",
                                    DrawChar::DiagRight => "/",
                                    DrawChar::DiagLeft => "\\",
                                    DrawChar::Text(_) => unreachable!(),
                                };
                                ctx.print(
                                    x as f64,
                                    (app.canvas_height as f64 - 1.0 - ((y - app.scroll_y) as f64)),
                                    Span::styled(char_to_draw, Style::default().fg(app.current_color)),
                                );
                            }
                        }
                    }
                }
            }

            // Only draw cursor if it's visible
            if app.cursor_y >= app.scroll_y as f64 && app.cursor_y < (app.scroll_y + app.canvas_height) as f64 {
                ctx.draw(&Points {
                    coords: &[(app.cursor_x, (app.canvas_height as f64 - 1.0 - (app.cursor_y - app.scroll_y as f64)))],
                    color: match app.mode {
                        AppMode::Drawing => Color::Rgb(255, 105, 180), // Hot pink
                        AppMode::Selection => Color::Yellow,
                        AppMode::ColorSelection => Color::Cyan,
                        AppMode::TypstInput => Color::Green,
                        AppMode::CoordinateInput => Color::Magenta,
                        AppMode::Settings => Color::Blue,
                        AppMode::PdfRender => Color::White,
                    },
                });
            }

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
    
    // Render settings popup if in settings mode
    if app.mode == AppMode::Settings {
        let settings_content = format!(
            "Settings (Press key to toggle):\n\n[a] Axes: {}\n[n] Grid Snap: {}\n[d] Continuous: {}\n\nCoordinate System:\n[1] Cartesian {}\n[2] Polar {}\n[3] Cylindrical {}\n\nPress ? or Esc to close",
            if app.show_axes { "ON" } else { "OFF" },
            if app.grid_snap { "ON" } else { "OFF" },
            if app.continuous_draw { "ON" } else { "OFF" },
            if matches!(app.coordinate_system, CoordinateSystem::Cartesian) { "◉" } else { "○" },
            if matches!(app.coordinate_system, CoordinateSystem::Polar) { "◉" } else { "○" },
            if matches!(app.coordinate_system, CoordinateSystem::Cylindrical) { "◉" } else { "○" },
        );
        
        let settings_widget = Paragraph::new(settings_content)
            .block(Block::default()
                .title("Settings")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Rgb(100, 149, 237)).bg(Color::Black)))
            .style(Style::default().bg(Color::Black).fg(Color::White));
        f.render_widget(settings_widget, chunks[1]);
    }

    let status_text = match app.mode {
        AppMode::Drawing => {
            let char_name = match &app.current_char {
                DrawChar::Point => "point",
                DrawChar::Horizontal => "horizontal",
                DrawChar::Vertical => "vertical", 
                DrawChar::Cross => "cross",
                DrawChar::DiagRight => "diag-right",
                DrawChar::DiagLeft => "diag-left",
                DrawChar::Text(ch) => &format!("text({})", ch),
            };
            format!("hjkl:move | space:draw | i:text | g:goto | s:save | x:color | J/K:scroll | ?:settings | q:quit | Drawing: {}", char_name)
        }
        AppMode::Selection => "Selection mode - press any key to jump to that position, Esc to cancel".to_string(),
        AppMode::ColorSelection => format!("Color (hex): {} | Enter to apply, Esc to cancel", app.color_input),
        AppMode::TypstInput => format!("Typst mode: {} | Enter to place, use $ for math, Backspace to edit, Esc to exit", app.text_buffer),
        AppMode::Settings => "Settings mode - use keys shown in popup to toggle options, ? or Esc to close".to_string(),
        AppMode::PdfRender => "PDF Render mode - viewing compiled PDF. Press r or Esc to return to drawing".to_string(),
        AppMode::CoordinateInput => {
            let hint = match app.coordinate_system {
                CoordinateSystem::Cartesian => "x,y",
                CoordinateSystem::Polar => "r,θ(deg)",
                CoordinateSystem::Cylindrical => "ρ,θ(deg),z",
            };
            format!("Go to ({}): {} | Enter to move, Esc to cancel", hint, app.coordinate_input)
        }
    };

    let status = Paragraph::new(status_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Rgb(100, 149, 237)).bg(Color::Black)))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(status, main_chunks[1]);
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