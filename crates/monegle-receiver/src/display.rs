use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::info;

/// Terminal display component
pub struct TerminalDisplay {
    fps: u8,
    width: u16,
    height: u16,
    stream_id: String,
}

impl TerminalDisplay {
    pub fn new(fps: u8, width: u16, height: u16, stream_id: String) -> Self {
        Self {
            fps,
            width,
            height,
            stream_id,
        }
    }

    /// Start display loop
    pub async fn start_display_loop(
        self,
        mut rx: mpsc::Receiver<String>,
    ) -> Result<()> {
        info!("Starting terminal display");

        // Check if first frame contains ANSI codes
        let first_frame = rx.recv().await.ok_or_else(|| anyhow::anyhow!("No frames received"))?;
        let has_ansi_codes = first_frame.contains("\x1b[");

        if has_ansi_codes {
            info!("Detected ANSI color codes - using direct terminal output");
            self.start_direct_display_loop(rx, first_frame).await
        } else {
            info!("No ANSI codes detected - using ratatui display");
            self.start_ratatui_display_loop(rx, first_frame).await
        }
    }

    /// Direct terminal output (for ANSI colored frames)
    async fn start_direct_display_loop(
        self,
        mut rx: mpsc::Receiver<String>,
        first_frame: String,
    ) -> Result<()> {
        use std::io::Write;

        let mut stdout = io::stdout();
        let mut frame_count = 0u64;
        let mut fps_counter = FpsCounter::new();
        let start_time = std::time::Instant::now();

        // Display first frame
        print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top-left
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║  Monegle Stream Receiver - Press Ctrl+C to stop      ║");
        println!("╠════════════════════════════════════════════════════════╣");
        println!("{}", first_frame);
        println!("╚════════════════════════════════════════════════════════╝");
        println!("Frame: {} | FPS: {:.1} | Time: {:.1}s",
            frame_count, fps_counter.fps(), start_time.elapsed().as_secs_f32());
        stdout.flush()?;

        frame_count += 1;
        fps_counter.tick();

        // Display subsequent frames
        while let Some(frame) = rx.recv().await {
            frame_count += 1;
            fps_counter.tick();

            // Clear screen and redraw
            print!("\x1B[2J\x1B[H"); // Clear screen and move to top
            println!("╔════════════════════════════════════════════════════════╗");
            println!("║  Monegle Stream Receiver - Press Ctrl+C to stop      ║");
            println!("╠════════════════════════════════════════════════════════╣");
            println!("{}", frame);
            println!("╚════════════════════════════════════════════════════════╝");
            println!("Frame: {} | FPS: {:.1} | Time: {:.1}s | Stream: {}",
                frame_count, fps_counter.fps(), start_time.elapsed().as_secs_f32(), self.stream_id);
            stdout.flush()?;

            // Small delay for frame rate
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }

        info!("Direct display stopped after {} frames", frame_count);
        Ok(())
    }

    /// Ratatui display (for monochrome frames)
    async fn start_ratatui_display_loop(
        self,
        mut rx: mpsc::Receiver<String>,
        first_frame: String,
    ) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

        let mut current_frame = first_frame;
        let mut frame_count = 1u64;
        let mut fps_counter = FpsCounter::new();
        fps_counter.tick();

        let result = loop {
            // Check for user input (non-blocking)
            if event::poll(std::time::Duration::from_millis(0))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        info!("User requested quit");
                        break Ok(());
                    }
                }
            }

            // Try to get next frame (non-blocking)
            if let Ok(frame) = rx.try_recv() {
                current_frame = frame;
                frame_count += 1;
                fps_counter.tick();
            }

            // Render
            terminal.draw(|f| {
                self.render_frame(
                    f,
                    &current_frame,
                    frame_count,
                    fps_counter.fps(),
                );
            })?;

            // Small delay to avoid busy-waiting
            tokio::time::sleep(std::time::Duration::from_millis(16)).await; // ~60 Hz refresh
        };

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        info!("Terminal display stopped after {} frames", frame_count);

        result
    }

    /// Render a single frame
    fn render_frame(
        &self,
        f: &mut Frame,
        ascii_frame: &str,
        frame_count: u64,
        current_fps: f32,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),      // Header
                Constraint::Min(10),        // Main frame area
                Constraint::Length(3),      // Footer
            ])
            .split(f.size());

        // Header
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Monegle Stream Receiver", Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(
                format!("FPS: {:.1}/{}", current_fps, self.fps),
                Style::default().fg(Color::Green),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

        f.render_widget(header, chunks[0]);

        // Main frame area
        let frame_widget = Paragraph::new(ascii_frame)
            .block(Block::default().borders(Borders::ALL).title("Stream"))
            .alignment(Alignment::Left);

        f.render_widget(frame_widget, chunks[1]);

        // Footer with stats
        let footer = Paragraph::new(Line::from(vec![
            Span::raw("Stream: "),
            Span::styled(&self.stream_id, Style::default().fg(Color::Yellow)),
            Span::raw(format!(" | Frames: {} | ", frame_count)),
            Span::styled("Press 'q' to quit", Style::default().fg(Color::Red)),
        ]))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

        f.render_widget(footer, chunks[2]);
    }
}

/// FPS counter
struct FpsCounter {
    frame_times: Vec<Instant>,
    window_size: usize,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            frame_times: Vec::new(),
            window_size: 30, // Calculate FPS over last 30 frames
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.frame_times.push(now);

        if self.frame_times.len() > self.window_size {
            self.frame_times.remove(0);
        }
    }

    fn fps(&self) -> f32 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }

        let elapsed = self.frame_times.last().unwrap()
            .duration_since(*self.frame_times.first().unwrap())
            .as_secs_f32();

        if elapsed == 0.0 {
            return 0.0;
        }

        (self.frame_times.len() - 1) as f32 / elapsed
    }
}
