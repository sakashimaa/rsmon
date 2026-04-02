use crossterm::event::{self, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline, TableState};
use ratatui::widgets::{Row, Table};
use ratatui::{DefaultTerminal, Frame};
use std::collections::VecDeque;
use std::process;
use std::{fs, time::Duration};

struct MemInfo {
    total: u64,
    available: u64,
}

struct CpuSnapshot {
    idle: u64,
    total: u64,
}

struct NetSnapshot {
    rx_bytes: u64,
    tx_bytes: u64,
}

struct NetUsage {
    rx_per_sec: u64,
    tx_per_sec: u64,
}

struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_usage: f64,
    mem_kb: u64,
}

struct App {
    mem: MemInfo,
    cpu_prev: CpuSnapshot,
    net_prev: NetSnapshot,
    cpu_usage: f64,
    net_usage: NetUsage,
    processes: Vec<ProcessInfo>,
    table_state: TableState,
    cpu_history: VecDeque<u64>,
    filter: String,
    filtering: bool,
}

impl MemInfo {
    fn usage_percent(&self) -> f64 {
        let used = self.total - self.available;
        (used as f64 / self.total as f64) * 100.0
    }
}

impl CpuSnapshot {
    fn new() -> CpuSnapshot {
        let contents = fs::read_to_string("/proc/stat").expect("Failed to read /proc/stat");
        let first_line = contents.lines().next().expect("Empty /proc/stat");

        let values: Vec<u64> = first_line
            .split_whitespace()
            .skip(1)
            .map(|v| v.parse().unwrap_or(0))
            .collect();

        let idle = values[3] + values[4];
        let total: u64 = values.iter().sum();

        CpuSnapshot { idle, total }
    }
}

impl ProcessInfo {}

impl App {
    fn new() -> App {
        App {
            mem: parse_mem_info(),
            cpu_prev: CpuSnapshot::new(),
            net_prev: NetSnapshot {
                rx_bytes: 0,
                tx_bytes: 0,
            },
            net_usage: NetUsage {
                rx_per_sec: 0,
                tx_per_sec: 0,
            },
            cpu_usage: 0.0,
            processes: vec![],
            table_state: TableState::default().with_selected(Some(0)),
            cpu_history: VecDeque::with_capacity(60),
            filter: String::from(""),
            filtering: false,
        }
    }

    fn handle_input(&mut self) -> std::io::Result<bool> {
        if event::poll(Duration::from_millis(1000))? {
            if let event::Event::Key(key) = event::read()? {
                if self.filtering {
                    match key.code {
                        KeyCode::Esc => {
                            self.filtering = false;
                            self.filter.clear();
                        }
                        KeyCode::Enter => {
                            self.filtering = false;
                        }
                        KeyCode::Backspace => {
                            self.filter.pop();
                        }
                        KeyCode::Char(c) => {
                            self.filter.push(c);
                        }
                        _ => {}
                    }

                    return Ok(false);
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Char('d') => {
                        if let Some(selected) = self.table_state.selected() {
                            if let Some(proc) = self.processes.get(selected) {
                                unsafe {
                                    libc::kill(proc.pid as i32, libc::SIGTERM);
                                }
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => self.table_state.select_next(),
                    KeyCode::Up | KeyCode::Char('k') => self.table_state.select_previous(),
                    KeyCode::Char('/') => self.filtering = true,
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    fn update(&mut self) {
        self.mem = parse_mem_info();
        let cpu_curr = CpuSnapshot::new();
        self.cpu_usage = cpu_usage(&self.cpu_prev, &cpu_curr);
        self.cpu_prev = cpu_curr;

        self.cpu_history.push_back(self.cpu_usage as u64);
        if self.cpu_history.len() > 60 {
            self.cpu_history.pop_front();
        }

        self.processes = parse_processes();
        self.processes.retain(|p| p.mem_kb > 0);
        self.processes.sort_by(|a, b| b.mem_kb.cmp(&a.mem_kb));

        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            self.processes
                .retain(|p| p.name.to_lowercase().contains(&filter_lower));
        }

        let net_curr = parse_net_stats();
        self.net_usage = NetUsage {
            rx_per_sec: net_curr.rx_bytes - self.net_prev.rx_bytes,
            tx_per_sec: net_curr.tx_bytes - self.net_prev.tx_bytes,
        };
        self.net_prev = net_curr;
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(5),
            ])
            .split(frame.area());

        let mem_percent = self.mem.usage_percent();
        let mem_gauge = Gauge::default()
            .block(Block::default().title("Memory").borders(Borders::ALL))
            .gauge_style(Style::default().fg(color_by_thresold(mem_percent)))
            .percent(mem_percent as u16);

        let cpu_usage = self.cpu_usage;
        let cpu_gauge = Gauge::default()
            .block(Block::default().title("CPU").borders(Borders::ALL))
            .gauge_style(Style::default().fg(color_by_thresold(cpu_usage)))
            .percent(cpu_usage as u16);

        let history: Vec<u64> = self.cpu_history.iter().copied().collect();
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title("CPU History (60s)")
                    .borders(Borders::ALL),
            )
            .data(&history)
            .max(100)
            .style(Style::default().fg(Color::Green));

        let header = Row::new(["PID", "Name", "CPU", "Mem KB"])
            .style(Style::new().bold())
            .bottom_margin(1);

        let rows = self.processes.iter().map(|proc| {
            Row::new([
                proc.pid.to_string(),
                proc.name.clone(),
                format!("{:.1}%", proc.cpu_usage),
                format!("{} KB", proc.mem_kb),
            ])
        });

        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];

        let table_title = if self.filter.is_empty() {
            "Processes".to_string()
        } else {
            format!("Processes [filter: {}]", self.filter)
        };

        let table = Table::new(rows, widths)
            .block(Block::default().title(table_title).borders(Borders::ALL))
            .header(header)
            .column_spacing(1)
            .style(Color::Red)
            .row_highlight_style(Style::new().on_black().bold())
            .column_highlight_style(Color::Gray)
            .cell_highlight_style(Style::new().reversed().black());

        let net_text = format!(
            "↓ {}/s  ↑ {}/s",
            format_bytes(self.net_usage.rx_per_sec),
            format_bytes(self.net_usage.tx_per_sec)
        );

        let net_widget =
            Paragraph::new(net_text).block(Block::default().title("Network").borders(Borders::ALL));

        frame.render_widget(mem_gauge, chunks[0]);
        frame.render_stateful_widget(table, chunks[1], &mut self.table_state);
        frame.render_widget(cpu_gauge, chunks[2]);
        frame.render_widget(sparkline, chunks[3]);
        frame.render_widget(net_widget, chunks[4]);
    }
}

fn cpu_usage(prev: &CpuSnapshot, curr: &CpuSnapshot) -> f64 {
    let total_delta = curr.total - prev.total;
    let idle_delta = curr.idle - prev.idle;
    if total_delta == 0 {
        return 0.0;
    }
    (1.0 - idle_delta as f64 / total_delta as f64) * 100.0
}

fn parse_net_stats() -> NetSnapshot {
    let contents = fs::read_to_string("/proc/net/dev").expect("Failed to read /proc/net/dev");

    let mut rx_total = 0u64;
    let mut tx_total = 0u64;

    for line in contents.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        if parts[0].starts_with("lo:") {
            continue;
        }

        rx_total += parts[1].parse::<u64>().unwrap_or(0);
        tx_total += parts[9].parse::<u64>().unwrap_or(0);
    }

    NetSnapshot {
        rx_bytes: rx_total,
        tx_bytes: tx_total,
    }
}

fn parse_mem_info() -> MemInfo {
    let contents = fs::read_to_string("/proc/meminfo").expect("Failed to read /proc/meminfo");

    let mut total = 0;
    let mut available = 0;

    for line in contents.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "MemTotal:" => total = parts[1].parse().unwrap_or(0),
            "MemAvailable:" => available = parts[1].parse().unwrap_or(0),
            _ => {}
        }
    }

    MemInfo { total, available }
}

fn parse_processes() -> Vec<ProcessInfo> {
    let paths = fs::read_dir("/proc").unwrap_or_else(|_| {
        eprintln!("Failed to list /proc contents");
        process::exit(1);
    });

    let mut result: Vec<ProcessInfo> = vec![];
    for path in paths {
        let path = match path {
            Ok(e) => e.path(),
            Err(e) => {
                eprintln!("Invalid path: {}", e);
                continue;
            }
        };

        let file_name = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() && file_name.parse::<u32>().is_ok() {
            let pid: u32 = file_name.parse().unwrap();
            let stat_path = path.join("stat");
            let statm_path = path.join("statm");

            let stat_content = match fs::read_to_string(stat_path) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let first_stat_line = match stat_content.lines().next() {
                Some(line) => line,
                None => continue,
            };

            let statm_content = match fs::read_to_string(statm_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let first_statm_line = match statm_content.lines().next() {
                Some(line) => line,
                None => continue,
            };

            let stat_values: Vec<&str> = first_stat_line.split_whitespace().collect();
            let statm_values: Vec<&str> = first_statm_line.split_whitespace().collect();

            result.push(ProcessInfo {
                pid,
                name: stat_values[1]
                    .trim_matches(|c| c == '(' || c == ')')
                    .to_string(),
                cpu_usage: 0.0,
                mem_kb: statm_values[1].parse::<u64>().unwrap_or(0) * 4,
            })
        }
    }

    result
}

fn color_by_thresold(value: f64) -> Color {
    if value > 80.0 {
        Color::Red
    } else if value > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new();
    loop {
        app.update();
        terminal.draw(|frame| app.render(frame))?;

        if app.handle_input()? {
            break Ok(());
        }
    }
}
