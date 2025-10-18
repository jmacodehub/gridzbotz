//! 🎮 Live Interactive CLI Dashboard - Real-Time Trading Monitor
//! Like htop but for grid trading! Press 'q' to quit, 'r' to refresh.

use std::io::{self, stdout};
use std::time::{Duration, Instant};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
    Terminal,
};
use glob::glob;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct TestResult {
    name: String,
    final_roi: f64,
    sharpe_ratio: f64,
    total_fills: usize,
    filtered_trades: usize,
    grid_spacing: f64,
    grid_levels: u32,
}

struct DashboardState {
    results: Vec<TestResult>,
    selected: usize,
    start_time: Instant,
    roi_history: Vec<f64>,
    last_load_time: Instant,
}

impl DashboardState {
    fn new() -> Self {
        Self {
            results: Vec::new(),
            selected: 0,
            start_time: Instant::now(),
            roi_history: vec![0.0; 50],
            last_load_time: Instant::now(),
        }
    }
    
    fn load_results(&mut self) {
        self.results.clear();
        
        // Safe glob with proper error handling
        match glob("results/giga_v*_test_*.json") {
            Ok(paths) => {
                for entry in paths {
                    if let Ok(path) = entry {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(data) = serde_json::from_str::<Vec<TestResult>>(&content) {
                                self.results.extend(data);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // No results found yet
            }
        }
        
        // Sort by ROI descending
        self.results.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap_or(std::cmp::Ordering::Equal));
        
        // Update ROI history for sparkline
        if !self.results.is_empty() {
            self.roi_history.rotate_left(1);
            let avg_roi = self.results.iter().map(|r| r.final_roi).sum::<f64>() / self.results.len() as f64;
            *self.roi_history.last_mut().unwrap() = avg_roi;
        }
        
        self.last_load_time = Instant::now();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut state = DashboardState::new();
    state.load_results();
    
    let result = run_app(&mut terminal, &mut state);
    
    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }
    
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, state: &mut DashboardState) -> io::Result<()> {
    loop {
        // Auto-refresh every 2 seconds
        if state.last_load_time.elapsed() > Duration::from_secs(2) {
            state.load_results();
        }
        
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Length(5),  // Stats
                    Constraint::Min(10),    // Results list
                    Constraint::Length(5),  // ROI chart
                    Constraint::Length(3),  // Footer
                ])
                .split(f.area());
            
            render_header(f, chunks[0], state);
            render_stats(f, chunks[1], state);
            render_results(f, chunks[2], state);
            render_chart(f, chunks[3], state);
            render_footer(f, chunks[4]);
        })?;
        
        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up => {
                        if state.selected > 0 {
                            state.selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if state.selected < state.results.len().saturating_sub(1) {
                            state.selected += 1;
                        }
                    }
                    KeyCode::Char('r') => {
                        state.load_results();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn render_header(f: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let elapsed = state.start_time.elapsed();
    let title = format!(
        "🔥💎 PROJECT FLASH V2.5 - LIVE DASHBOARD 🚀💎🔥  |  Uptime: {:02}:{:02}:{:02}  |  Tests: {}",
        elapsed.as_secs() / 3600,
        (elapsed.as_secs() % 3600) / 60,
        elapsed.as_secs() % 60,
        state.results.len()
    );
    
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn render_stats(f: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    if state.results.is_empty() {
        let no_data = Paragraph::new("📊 No data yet... Run some tests!\n\nTry: cargo run --example giga_test --release")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(no_data, area);
        return;
    }
    
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);
    
    let avg_roi = state.results.iter().map(|r| r.final_roi).sum::<f64>() / state.results.len() as f64;
    let positive = state.results.iter().filter(|r| r.final_roi > 0.0).count();
    let win_rate = (positive as f64 / state.results.len() as f64) * 100.0;
    let total_filtered = state.results.iter().map(|r| r.filtered_trades).sum::<usize>();
    let best_roi = state.results.first().map(|r| r.final_roi).unwrap_or(0.0);
    
    // Avg ROI Gauge
    let roi_color = if avg_roi > 0.0 { Color::Green } else { Color::Red };
    let roi_ratio = (avg_roi.abs() / 10.0).min(1.0);
    let roi_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Avg ROI"))
        .gauge_style(Style::default().fg(roi_color))
        .ratio(roi_ratio)
        .label(format!("{:.2}%", avg_roi));
    f.render_widget(roi_gauge, chunks[0]);
    
    // Win Rate Gauge
    let win_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Win Rate"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .ratio(win_rate / 100.0)
        .label(format!("{:.1}%", win_rate));
    f.render_widget(win_gauge, chunks[1]);
    
    // Filtered Trades
    let filtered_text = Paragraph::new(format!("\n  {} trades\n  saved!", total_filtered))
        .style(Style::default().fg(Color::Red))
        .block(Block::default().borders(Borders::ALL).title("Filtered"));
    f.render_widget(filtered_text, chunks[2]);
    
    // Best ROI
    let best_text = Paragraph::new(format!("\n  {:.2}%", best_roi))
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title("Best ROI"));
    f.render_widget(best_text, chunks[3]);
}

fn render_results(f: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let items: Vec<ListItem> = state.results.iter().take(20).enumerate().map(|(i, r)| {
        let color = if r.final_roi > 0.0 { Color::Green } else { Color::Red };
        let selected = if i == state.selected { "▶ " } else { "  " };
        
        let line = format!(
            "{}{:<23} ROI:{:>7.2}% Sharpe:{:>5.2} {:>5.2}%/{:>2}lvl F:{:>3} Filt:{:>3}",
            selected,
            truncate(&r.name, 23),
            r.final_roi,
            r.sharpe_ratio,
            r.grid_spacing,
            r.grid_levels,
            r.total_fills,
            r.filtered_trades
        );
        
        ListItem::new(Line::from(Span::styled(line, Style::default().fg(color))))
    }).collect();
    
    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Test Results (↑↓ navigate | r refresh | q quit)"))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD));
    
    f.render_widget(list, area);
}

fn render_chart(f: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    // Convert ROI history to sparkline data (0-100 range)
    let data: Vec<u64> = state.roi_history.iter()
        .map(|v| {
            let normalized = (v + 10.0) * 5.0; // Map -10 to +10 ROI to 0-100 range
            normalized.max(0.0).min(100.0) as u64
        })
        .collect();
    
    let sparkline = Sparkline::default()
        .block(Block::default()
            .borders(Borders::ALL)
            .title("📈 ROI Trend (last 50 updates)"))
        .data(&data)
        .style(Style::default().fg(Color::Cyan));
    
    f.render_widget(sparkline, area);
}

fn render_footer(f: &mut ratatui::Frame, area: Rect) {
    let footer_text = "Controls: [q/Esc] Quit | [r] Refresh | [↑↓] Navigate | Auto-refresh: 2s";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        format!("{:<width$}", s, width = max_len)
    }
}
