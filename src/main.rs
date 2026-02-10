use anyhow::Result;
use chrono::Local;
use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use default_net::get_default_gateway; 
use ratatui::{
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, LegendPosition},
};
use serde::Serialize;
use std::{fs::OpenOptions, io, time::Duration, net::IpAddr};
use tokio::sync::mpsc;
use rand::seq::SliceRandom;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};

const TARGET_POOL: &[&str] = &[
    "1.1.1.1", "8.8.8.8", "9.9.9.9", "208.67.222.222", "1.0.0.1", "8.8.4.4",
];

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum PingMode {
    Gaming,
    Standard,
    Monitor,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    target: Option<String>,

    #[arg(short, long, value_enum, default_value_t = PingMode::Gaming)]
    mode: PingMode,

    #[arg(short, long)]
    duration: Option<String>,

    #[arg(short, long)]
    interval: Option<String>,

    #[arg(long, default_value_t = false)]
    no_gateway: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum SourceType {
    Internet,
    Gateway,
}

#[derive(Debug)]
struct PingUpdate {
    source: SourceType,
    latency: f64, 
}

#[derive(Debug, Serialize, Clone)]
struct PingRecord {
    timestamp: String,
    target_type: String, 
    target_ip: String,
    latency_ms: Option<f64>, 
    status: String, 
}

fn parse_duration_string(s: &str) -> Option<Duration> {
    let digits: String = s.chars().take_while(|c| c.is_digit(10)).collect();
    let unit: String = s.chars().skip(digits.len()).collect();
    if let Ok(val) = digits.parse::<u64>() {
        match unit.trim() {
            "ms" => Some(Duration::from_millis(val)),
            "s" | "" => Some(Duration::from_secs(val)),
            "m" => Some(Duration::from_secs(val * 60)),
            "h" => Some(Duration::from_secs(val * 3600)),
            _ => None,
        }
    } else { None }
}

async fn run_pinger(
    target_ip: IpAddr,
    interval: Duration,
    source_type: SourceType,
    tx: mpsc::Sender<PingUpdate>,
) {
    let client = match Client::new(&Config::default()) {
        Ok(c) => c,
        Err(_) => return, 
    };

    let mut pinger = client.pinger(target_ip, PingIdentifier(rand::random())).await;
    let mut seq_cnt = 0u16;
    let mut interval_timer = tokio::time::interval(interval);

    loop {
        interval_timer.tick().await;
        
        let payload = [0; 8];
        match pinger.ping(PingSequence(seq_cnt), &payload).await {
            Ok((_, duration)) => {
                let ms = duration.as_secs_f64() * 1000.0;
                let _ = tx.send(PingUpdate { source: source_type.clone(), latency: ms }).await;
            }
            Err(_) => {
                let _ = tx.send(PingUpdate { source: source_type.clone(), latency: -1.0 }).await;
            }
        };
        seq_cnt = seq_cnt.wrapping_add(1);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let (default_interval_ms, default_mode_name) = match args.mode {
        PingMode::Gaming => (200, "GAMING"),
        PingMode::Standard => (1000, "STANDARD"),
        PingMode::Monitor => (5000, "MONITOR"),
    };

    let (ping_interval, mode_display_name) = if let Some(i_str) = args.interval {
        if let Some(d) = parse_duration_string(&i_str) { (d, "USER SPECIFIED".to_string()) } 
        else { (Duration::from_millis(default_interval_ms), default_mode_name.to_string()) }
    } else { (Duration::from_millis(default_interval_ms), default_mode_name.to_string()) };

    let ping_interval_ms = ping_interval.as_millis() as u64;
    let time_factor = ping_interval_ms as f64 / 1000.0;
    let max_duration = args.duration.as_ref().and_then(|d| parse_duration_string(d));

    let (target_host, target_source_label, target_source_color) = match args.target {
        Some(t) => (t, "User Specified", Color::Cyan),
        None => {
            let mut rng = rand::thread_rng();
            (TARGET_POOL.choose(&mut rng).unwrap_or(&"8.8.8.8").to_string(), "Randomized Default", Color::Magenta)
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
            Ok(gw) => {
                match gw.ip_addr.to_string().parse::<IpAddr>() {
                    Ok(ip) => Some(ip),
                    Err(_) => None,
                }
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
            let popup_area = centered_rect(70, 85, area);

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
                    disable_raw_mode()?; execute!(terminal.backend_mut(), LeaveAlternateScreen)?; return Ok(());
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

    let mut internet_points: Vec<(f64, f64)> = vec![];
    let mut internet_jitter_points: Vec<(f64, f64)> = vec![];
    let mut gateway_points: Vec<(f64, f64)> = vec![];
    let mut gateway_jitter_points: Vec<(f64, f64)> = vec![];
    let mut loss_points_net: Vec<(f64, f64)> = vec![]; 
    let mut loss_points_gw: Vec<(f64, f64)> = vec![]; 
    
    let app_start_time = Local::now(); 
    let mut zoom_window_seconds = if ping_interval_ms <= 200 { 60.0 } else { 300.0 };
    let mut x_counter = 0.0; 
    let mut scroll_offset_seconds = 0.0; 
    
    let mut all_latencies_net: Vec<f64> = vec![];
    let mut last_latency_net = 0.0;
    let mut current_jitter_net = 0.0;
    let mut total_count_net = 0;
    let mut loss_count_net = 0;
    let mut spikes_minor_net = 0; 
    let mut spikes_major_net = 0; 

    let mut all_latencies_gw: Vec<f64> = vec![];
    let mut last_latency_gw = 0.0;
    let mut current_jitter_gw = 0.0;
    let mut total_count_gw = 0; 
    let mut loss_count_gw = 0; 
    let mut spikes_minor_gw = 0;
    let mut spikes_major_gw = 0;

    let mut is_paused = false;
    let mut is_finished = false;
    let mut recorded_duration_sec = 0.0; 

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(10), Constraint::Length(3), Constraint::Length(1)]).split(f.area());
            let current_time_seconds = x_counter * time_factor;
            let view_end_sec = if current_time_seconds - scroll_offset_seconds < 0.0 { 0.0 } else { current_time_seconds - scroll_offset_seconds };
            let view_start_sec = if view_end_sec - zoom_window_seconds < 0.0 { 0.0 } else { view_end_sec - zoom_window_seconds };
            let view_start_time_abs = app_start_time + chrono::Duration::milliseconds((view_start_sec * 1000.0) as i64);
            let view_end_time_abs = app_start_time + chrono::Duration::milliseconds((view_end_sec * 1000.0) as i64);
            let status_text = if is_finished { "[FINISHED]" } else if is_paused { "[PAUSED]" } else { "[LIVE]" };
            
            let title_prefix = format!(" VASILI - {} ({}ms) - Target: {} -", mode_display_name, ping_interval_ms, target_host);
            let (title, title_color) = if scroll_offset_seconds > 0.0 { (format!("{} HISTORY (-{:.0}s) {} [ {} - {} ] ", title_prefix, scroll_offset_seconds, status_text, view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Yellow) } 
            else if is_paused || is_finished { (format!("{} {} [ {} - {} ] ", title_prefix, status_text, view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Magenta) } 
            else { (format!("{} LIVE [ {} - {} ] ", title_prefix, view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Green) };

            let legend_net_ping = format!("NET Ping ({:.1}ms)", last_latency_net);
            let legend_net_jitter = format!("NET Jitter ({:.1}ms)", current_jitter_net);
            let legend_net_loss = format!("NET Loss ({})", loss_count_net); 
            let legend_gw_ping = format!("Gateway Ping ({:.1}ms)", last_latency_gw);
            let legend_gw_jitter = format!("Gateway Jitter ({:.1}ms)", current_jitter_gw);
            let legend_gw_loss = format!("Gateway Loss ({})", loss_count_gw);

            let mut datasets = vec![
                Dataset::default().name(legend_net_ping).marker(symbols::Marker::Braille).style(Style::default().fg(Color::Green)).graph_type(GraphType::Line).data(&internet_points),
                Dataset::default().name(legend_net_jitter).marker(symbols::Marker::Braille).style(Style::default().fg(Color::Yellow)).graph_type(GraphType::Line).data(&internet_jitter_points),
                Dataset::default().name(legend_net_loss).marker(symbols::Marker::Dot).style(Style::default().fg(Color::Red)).graph_type(GraphType::Scatter).data(&loss_points_net)
            ];
            
            if has_gateway {
                datasets.push(Dataset::default().name(legend_gw_ping).marker(symbols::Marker::Braille).style(Style::default().fg(Color::Blue)).graph_type(GraphType::Line).data(&gateway_points));
                datasets.push(Dataset::default().name(legend_gw_jitter).marker(symbols::Marker::Braille).style(Style::default().fg(Color::Magenta)).graph_type(GraphType::Line).data(&gateway_jitter_points));
                datasets.push(Dataset::default().name(legend_gw_loss).marker(symbols::Marker::Dot).style(Style::default().fg(Color::Magenta)).graph_type(GraphType::Scatter).data(&loss_points_gw));
            }

            let chart = Chart::new(datasets)
                .block(Block::default().title(Span::styled(title, Style::default().fg(title_color).add_modifier(Modifier::BOLD)))
                .title_bottom(Line::from(format!(" Seconds (Zoom: {:.0}s) ", zoom_window_seconds)).alignment(Alignment::Center).style(Style::default().fg(Color::Gray))).borders(Borders::ALL))
                .legend_position(Some(LegendPosition::TopRight))
                .x_axis(Axis::default().style(Style::default().fg(Color::Gray)).bounds([view_start_sec, view_end_sec]))
                .y_axis(Axis::default().title("ms").style(Style::default().fg(Color::Gray)).bounds([0.0, 100.0]).labels(vec![Span::styled("0", Style::default()), Span::styled("50", Style::default()), Span::styled("100", Style::default().fg(Color::Red))]));
            f.render_widget(chart, chunks[0]);

            let calc_p_values = |latencies: &Vec<f64>| -> (f64, f64, f64) { if latencies.len() > 10 { let mut sorted = latencies.clone(); sorted.sort_by(|a, b| a.partial_cmp(b).unwrap()); let len = sorted.len(); (sorted[(len as f64 * 0.25) as usize], sorted[(len as f64 * 0.75) as usize], sorted[(len as f64 * 0.95) as usize]) } else { (0.0, 0.0, 0.0) } };
            let loss_percent_net = if total_count_net > 0 { (loss_count_net as f64 / total_count_net as f64) * 100.0 } else { 0.0 };
            let (p25_net, p75_net, p95_net) = calc_p_values(&all_latencies_net);
            let loss_percent_gw = if total_count_gw > 0 { (loss_count_gw as f64 / total_count_gw as f64) * 100.0 } else { 0.0 };
            let (p25_gw, p75_gw, p95_gw) = calc_p_values(&all_latencies_gw);
            let grade_net = if loss_percent_net >= 5.0 || p95_net >= 120.0 { "F" } else if loss_percent_net >= 2.0 || p95_net >= 60.0 { "C" } else if loss_percent_net >= 0.5 || p95_net >= 30.0 { "B" } else if loss_percent_net > 0.0  || p95_net >= 10.0 { "A" } else { "S" };
            let grade_color_net = match grade_net { "S"|"A" => Color::Green, "B" => Color::Cyan, "C" => Color::Yellow, _ => Color::Red };
            let grade_gw = if loss_percent_gw >= 5.0 || p95_gw >= 50.0 { "F" } else if loss_percent_gw >= 2.0 || p95_gw >= 30.0 { "C" } else if loss_percent_gw >= 0.5 || p95_gw >= 10.0 { "B" } else if loss_percent_gw > 0.0  || p95_gw >= 5.0 { "A" } else { "S" };
            let grade_color_gw = match grade_gw { "S"|"A" => Color::Green, "B" => Color::Cyan, "C" => Color::Yellow, _ => Color::Red };
            let limit_str = if let Some(max) = max_duration { format!("/{:02}:{:02}", max.as_secs()/60, max.as_secs()%60) } else { String::new() };
            let runtime_str = format!("{:02}:{:02}{}", (recorded_duration_sec as u64)/60, (recorded_duration_sec as u64)%60, limit_str);

            if has_gateway {
                let stats_chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(50), Constraint::Percentage(50)]).split(chunks[1]);
                let stats_spans_net = vec![Span::raw("Loss: "), Span::styled(format!("{:.1}% ", loss_percent_net), Style::default().fg(if loss_count_net == 0 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)), Span::raw("| P(25/75/95): "), Span::styled(format!("{:.0}/{:.0}/{:.0}ms ", p25_net, p75_net, p95_net), Style::default().fg(Color::Cyan)), Span::raw("| Spikes >30ms: "), Span::styled(format!("{} ", spikes_minor_net), Style::default().fg(if spikes_minor_net == 0 { Color::Green } else { Color::Yellow })), Span::raw("| >100ms: "), Span::styled(format!("{} ", spikes_major_net), Style::default().fg(if spikes_major_net == 0 { Color::Green } else { Color::Red })), Span::raw("| Grade: "), Span::styled(grade_net, Style::default().fg(grade_color_net).add_modifier(Modifier::BOLD))];
                f.render_widget(Paragraph::new(Line::from(stats_spans_net)).block(Block::default().borders(Borders::ALL).title(format!(" Stats (NET) - Time: {} ", runtime_str))).style(Style::default().fg(Color::White)), stats_chunks[0]);
                let stats_spans_gw = vec![Span::raw("Loss: "), Span::styled(format!("{:.1}% ", loss_percent_gw), Style::default().fg(if loss_count_gw == 0 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)), Span::raw("| P(25/75/95): "), Span::styled(format!("{:.0}/{:.0}/{:.0}ms ", p25_gw, p75_gw, p95_gw), Style::default().fg(Color::Cyan)), Span::raw("| Spikes >30ms: "), Span::styled(format!("{} ", spikes_minor_gw), Style::default().fg(if spikes_minor_gw == 0 { Color::Green } else { Color::Yellow })), Span::raw("| >100ms: "), Span::styled(format!("{} ", spikes_major_gw), Style::default().fg(if spikes_major_gw == 0 { Color::Green } else { Color::Red })), Span::raw("| Grade: "), Span::styled(grade_gw, Style::default().fg(grade_color_gw).add_modifier(Modifier::BOLD))];
                f.render_widget(Paragraph::new(Line::from(stats_spans_gw)).block(Block::default().borders(Borders::ALL).title(" Stats (GATEWAY) ")).style(Style::default().fg(Color::White)), stats_chunks[1]);
            } else {
                let stats_spans = vec![Span::raw("Time: "), Span::styled(format!("{} | ", runtime_str), Style::default().fg(if is_finished { Color::Red } else { Color::White })), Span::raw("Loss: "), Span::styled(format!("{:.1}% ", loss_percent_net), Style::default().fg(if loss_count_net == 0 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)), Span::raw("| P(25/75/95): "), Span::styled(format!("{:.0}/{:.0}/{:.0}ms ", p25_net, p75_net, p95_net), Style::default().fg(Color::Cyan)), Span::raw("| Spikes >30ms: "), Span::styled(format!("{} ", spikes_minor_net), Style::default().fg(if spikes_minor_net == 0 { Color::Green } else { Color::Yellow })), Span::raw("| >100ms: "), Span::styled(format!("{} ", spikes_major_net), Style::default().fg(if spikes_major_net == 0 { Color::Green } else { Color::Red })), Span::raw("| Grade: "), Span::styled(grade_net, Style::default().fg(grade_color_net).add_modifier(Modifier::BOLD))];
                f.render_widget(Paragraph::new(Line::from(stats_spans)).block(Block::default().borders(Borders::ALL).title(" Statistics (NET) ")).style(Style::default().fg(Color::White)), chunks[1]);
            }
            f.render_widget(Paragraph::new(" [Q] Quit | [SPACE] Pause | [+/-] Zoom | [←/→] History ").style(Style::default().bg(Color::DarkGray).fg(Color::White)).alignment(Alignment::Center), chunks[2]);
        })?;

        tokio::select! {
            Some(update) = rx.recv() => {
                if !is_paused && !is_finished {
                    if let Some(max) = max_duration { if recorded_duration_sec >= max.as_secs_f64() { is_finished = true; continue; } }
                    let time_val = x_counter * time_factor;
                    match update.source {
                        SourceType::Internet => {
                            x_counter += 1.0; recorded_duration_sec += time_factor; total_count_net += 1;
                            if update.latency < 0.0 {
                                loss_count_net += 1; spikes_major_net += 1; loss_points_net.push((time_val, 100.0));
                                let _ = csv_writer.serialize(PingRecord { timestamp: Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string(), target_ip: target_host.to_string(), target_type: "Internet".to_string(), latency_ms: None, status: "TIMEOUT".to_string() });
                            } else {
                                let jitter = if last_latency_net == 0.0 { 0.0 } else { (update.latency - last_latency_net).abs() };
                                last_latency_net = update.latency; current_jitter_net = jitter; all_latencies_net.push(update.latency);
                                if update.latency >= 100.0 { spikes_major_net += 1; } else if update.latency >= 30.0 { spikes_minor_net += 1; }
                                internet_points.push((time_val, update.latency)); internet_jitter_points.push((time_val, jitter));
                                let _ = csv_writer.serialize(PingRecord { timestamp: Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string(), target_ip: target_host.to_string(), target_type: "Internet".to_string(), latency_ms: Some(update.latency), status: "OK".to_string() });
                            }
                        },
                        SourceType::Gateway => {
                            total_count_gw += 1;
                            if update.latency < 0.0 {
                                loss_count_gw += 1; spikes_major_gw += 1; loss_points_gw.push((time_val, 100.0));
                                let _ = csv_writer.serialize(PingRecord { timestamp: Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string(), target_ip: gateway_host_str.clone(), target_type: "Gateway".to_string(), latency_ms: None, status: "TIMEOUT".to_string() });
                            } else {
                                let jitter = if last_latency_gw == 0.0 { 0.0 } else { (update.latency - last_latency_gw).abs() };
                                last_latency_gw = update.latency; current_jitter_gw = jitter; all_latencies_gw.push(update.latency);
                                if update.latency >= 100.0 { spikes_major_gw += 1; } else if update.latency >= 30.0 { spikes_minor_gw += 1; }
                                gateway_points.push((time_val, update.latency)); gateway_jitter_points.push((time_val, jitter));
                                let _ = csv_writer.serialize(PingRecord { timestamp: Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string(), target_ip: gateway_host_str.clone(), target_type: "Gateway".to_string(), latency_ms: Some(update.latency), status: "OK".to_string() });
                            }
                        }
                    }
                    let _ = csv_writer.flush();
                }
            }
            event = async { tokio::task::spawn_blocking(|| event::poll(Duration::from_millis(50))).await } => { if let Ok(Ok(true)) = event { if let Event::Key(key) = event::read()? { match key.code { KeyCode::Char('q') => break, KeyCode::Char(' ') => if !is_finished { is_paused = !is_paused; }, KeyCode::Char('+') | KeyCode::Up => if zoom_window_seconds > 10.0 { zoom_window_seconds -= 10.0; }, KeyCode::Char('-') | KeyCode::Down => zoom_window_seconds += 10.0, KeyCode::Left => if scroll_offset_seconds < (x_counter * time_factor) { scroll_offset_seconds += 10.0; }, KeyCode::Right => { scroll_offset_seconds -= 10.0; if scroll_offset_seconds < 0.0 { scroll_offset_seconds = 0.0; } }, _ => {} } } } }
        }
    }
    disable_raw_mode()?; execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    println!("VASILI finished. Log saved to: {}", csv_path);
    Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)]).split(r);
    Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)]).split(popup_layout[1])[1]
}