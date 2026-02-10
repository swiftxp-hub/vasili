mod args;
mod app;
mod ui;
mod pinger;
mod utils;

use anyhow::Result;
use args::Args;
use app::App;
use pinger::{run_pinger, SourceType, PingUpdate};
use std::{fs::OpenOptions, io, time::Duration, net::IpAddr};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc;
use default_net::get_default_gateway;
use rand::seq::SliceRandom;
use clap::Parser;
use chrono::Local;

const TARGET_POOL: &[&str] = &[
    "1.1.1.1", "8.8.8.8", "9.9.9.9", "208.67.222.222", "1.0.0.1", "8.8.4.4",
];

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let (default_interval_ms, default_mode_name) = match args.mode {
        args::PingMode::Gaming => (200, "GAMING"),
        args::PingMode::Standard => (1000, "STANDARD"),
        args::PingMode::Monitor => (5000, "MONITOR"),
    };

    let (ping_interval, mode_display_name) = if let Some(i_str) = args.interval {
        if let Some(d) = args::parse_duration_string(&i_str) {
            (d, "USER SPECIFIED".to_string())
        } else {
            (Duration::from_millis(default_interval_ms), default_mode_name.to_string())
        }
    } else {
        (Duration::from_millis(default_interval_ms), default_mode_name.to_string())
    };

    let ping_interval_ms = ping_interval.as_millis() as u64;
    let max_duration = args.duration.as_ref().and_then(|d| args::parse_duration_string(d));

    let (target_host, target_source_label, target_source_color) = match args.target {
        Some(t) => (t, "User Specified", Color::Cyan),
        None => {
            let mut rng = rand::thread_rng();
            (
                TARGET_POOL.choose(&mut rng).unwrap_or(&"8.8.8.8").to_string(),
                "Randomized Default",
                Color::Magenta
            )
        }
    };

    let target_ip: IpAddr = match target_host.parse() {
        Ok(ip) => ip,
        Err(_) => "8.8.8.8".parse().unwrap(),
    };

    let gateway_ip_addr = if args.no_gateway {
        None
    } else {
        match get_default_gateway() {
            Ok(gw) => match gw.ip_addr.to_string().parse::<IpAddr>() {
                Ok(ip) => Some(ip),
                Err(_) => None,
            },
            Err(_) => None,
        }
    };

    let has_gateway = gateway_ip_addr.is_some();
    let gateway_host_str = gateway_ip_addr.map(|ip| ip.to_string()).unwrap_or_else(|| "N/A".to_string());

    let timestamp_str = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let safe_target = target_host.replace(":", "_");
    let csv_path = format!("vasili_{}_{}ms_{}.csv", timestamp_str, ping_interval_ms, safe_target);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            f.render_widget(Block::default().style(Style::default().bg(Color::Black)), area);
            let popup_area = utils::centered_rect(70, 85, area);

            let gw_line = if has_gateway {
                Line::from(vec![
                    Span::raw("Gateway (Next Hop): "),
                    Span::styled(format!("{} ", gateway_host_str), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::styled("(Auto-Detected)", Style::default().fg(Color::Green))
                ])
            } else if args.no_gateway {
                Line::from(Span::styled("Gateway: Disabled via argument", Style::default().fg(Color::Yellow)))
            } else {
                Line::from(Span::styled("Gateway: Not Found", Style::default().fg(Color::Red)))
            };

            let welcome_text = vec![
                Line::from(Span::styled("Welcome to VASILI", Style::default().add_modifier(Modifier::BOLD).fg(Color::Green))),
                Line::from(Span::styled("\"Give me a ping, Vasili. One ping only.\"", Style::default().add_modifier(Modifier::ITALIC).fg(Color::Cyan))),
                Line::from(""),
                Line::from(vec![Span::raw("Current Mode: "), Span::styled(mode_display_name.clone(), Style::default().fg(Color::Blue))]),
                Line::from(vec![Span::raw("Interval: "), Span::styled(format!("{}ms", ping_interval_ms), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))]),
                Line::from(vec![
                    Span::raw("Target: "),
                    Span::styled(format!("{} ", target_host), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("({})", target_source_label), Style::default().fg(target_source_color))
                ]),
                gw_line,
                Line::from(if let Some(d) = max_duration { format!("Limit: {}m {}s", d.as_secs()/60, d.as_secs()%60) } else { "Limit: Infinite".to_string() }),
                Line::from(""),
                Line::from(Span::styled("CONTROLS:", Style::default().fg(Color::Yellow))),
                Line::from("[+/-] Zoom Time Axis"),
                Line::from("[Left/Right] Scroll History"),
                Line::from("[Space] Pause / Resume"),
                Line::from("[Q] Quit"),
                Line::from(""),
                Line::from("Press [ENTER] to start monitoring"),
            ];

            let content_height = welcome_text.len() as u16;
            let block = Block::default().borders(Borders::ALL).title(" Config ").style(Style::default().fg(Color::White));
            f.render_widget(block.clone(), popup_area);
            let inner_area = block.inner(popup_area);
            let vertical_center = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(content_height), Constraint::Min(1)]).split(inner_area);
            f.render_widget(Paragraph::new(welcome_text).alignment(Alignment::Center), vertical_center[1]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Enter { break; }
                if key.code == KeyCode::Char('q') {
                    disable_raw_mode()?;
                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                    return Ok(());
                }
            }
        }
    }

    let file = OpenOptions::new().create(true).append(true).open(&csv_path)?;
    let mut csv_writer = csv::WriterBuilder::new().has_headers(false).from_writer(file);
    let (tx, mut rx) = mpsc::channel::<PingUpdate>(100);

    let tx_net = tx.clone();
    let interval_clone = ping_interval.clone();
    tokio::spawn(async move {
        run_pinger(target_ip, interval_clone, SourceType::Internet, tx_net).await;
    });

    if let Some(gw_ip) = gateway_ip_addr {
        let tx_gw = tx.clone();
        let interval_clone = ping_interval.clone();
        tokio::spawn(async move {
            run_pinger(gw_ip, interval_clone, SourceType::Gateway, tx_gw).await;
        });
    }

    let mut app = App::new(
        target_host,
        gateway_host_str.ne("N/A").then(|| gateway_host_str),
        ping_interval_ms,
        max_duration
    );

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        tokio::select! {
            Some(update) = rx.recv() => {
                if let Some(record) = app.on_ping(update.source, update.latency) {
                    let _ = csv_writer.serialize(record);
                    let _ = csv_writer.flush();
                }
            }
            event = async { tokio::task::spawn_blocking(|| event::poll(Duration::from_millis(50))).await } => {
                if let Ok(Ok(true)) = event {
                    if let Event::Key(key) = event::read()? {
                       app.on_key(key.code);
                    }
                }
            }
        }

        if app.should_quit || app.is_finished {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    println!("VASILI finished. Log saved to: {}", csv_path);
    Ok(())
}