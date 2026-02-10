use ratatui::{
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, LegendPosition},
};
use crate::app::{App, HostStats};
use chrono::Duration;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_chart(f, chunks[0], app);
    
    if app.gw_stats.is_some() {
        let stats_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);
        
        draw_host_stats(f, stats_chunks[0], &app.net_stats, "TARGET", app);
        if let Some(gw) = &app.gw_stats {
            draw_host_stats(f, stats_chunks[1], gw, "GATEWAY", app);
        }
    } else {
        draw_host_stats(f, chunks[1], &app.net_stats, "TARGET", app);
    }

    draw_footer(f, chunks[2], app);
}

fn draw_chart(f: &mut Frame, area: Rect, app: &App) {
    let current_time_seconds = app.x_counter * app.time_factor;
    let view_end_sec = (current_time_seconds - app.scroll_offset_seconds).max(0.0);
    let view_start_sec = (view_end_sec - app.zoom_window_seconds).max(0.0);
    
    let view_start_time_abs = app.start_time + Duration::milliseconds((view_start_sec * 1000.0) as i64);
    let view_end_time_abs = app.start_time + Duration::milliseconds((view_end_sec * 1000.0) as i64);

    let status_text = if app.is_finished { "[FINISHED]" } else if app.is_paused { "[PAUSED]" } else { "[LIVE]" };
    let title_prefix = format!(" VASILI ({}ms) - Target: {} -", app.configured_interval, app.net_stats.display_name);

    let (title, title_color) = if app.scroll_offset_seconds > 0.0 {
        (format!("{} HISTORY (-{:.0}s) {} [ {} - {} ] ", 
            title_prefix, app.scroll_offset_seconds, status_text, 
            view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Yellow)
    } else if app.is_paused || app.is_finished {
        (format!("{} {} [ {} - {} ] ", 
            title_prefix, status_text, 
            view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Magenta)
    } else {
        (format!("{} LIVE [ {} - {} ] ", 
            title_prefix, 
            view_start_time_abs.format("%H:%M:%S"), view_end_time_abs.format("%H:%M:%S")), Color::Green)
    };

    let mut datasets = Vec::new();

    let net_ping_legend = format!("TARGET Ping ({:.1}ms)", app.net_stats.last_latency);
    datasets.push(Dataset::default()
        .name(net_ping_legend)
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Green))
        .graph_type(GraphType::Line)
        .data(&app.net_stats.points));

    let net_jitter_legend = format!("TARGET Jitter ({:.1}ms)", app.net_stats.current_jitter);
    datasets.push(Dataset::default()
        .name(net_jitter_legend)
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Yellow))
        .graph_type(GraphType::Line)
        .data(&app.net_stats.jitter_points));

    let net_loss_legend = format!("TARGET Loss ({})", app.net_stats.loss_count);
    datasets.push(Dataset::default()
        .name(net_loss_legend)
        .marker(symbols::Marker::Block)
        .style(Style::default().fg(Color::Red))
        .graph_type(GraphType::Scatter)
        .data(&app.net_stats.loss_points));

    if let Some(gw) = &app.gw_stats {
        let gw_ping_legend = format!("GATEWAY Ping ({:.1}ms)", gw.last_latency);
        datasets.push(Dataset::default()
            .name(gw_ping_legend)
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Blue))
            .graph_type(GraphType::Line)
            .data(&gw.points));

        let gw_jitter_legend = format!("GATEWAY Jitter ({:.1}ms)", gw.current_jitter);
        datasets.push(Dataset::default()
            .name(gw_jitter_legend)
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Magenta))
            .graph_type(GraphType::Line)
            .data(&gw.jitter_points));
            
        let gw_loss_legend = format!("GATEWAY Loss ({})", gw.loss_count);
        datasets.push(Dataset::default()
            .name(gw_loss_legend)
            .marker(symbols::Marker::Block)
            .style(Style::default().fg(Color::Magenta))
            .graph_type(GraphType::Scatter)
            .data(&gw.loss_points));
    }

    let chart = Chart::new(datasets)
        .block(Block::default()
            .title(Span::styled(title, Style::default().fg(title_color).add_modifier(Modifier::BOLD)))
            .title_bottom(Line::from(format!(" Seconds (Zoom: {:.0}s) ", app.zoom_window_seconds)).alignment(Alignment::Center).style(Style::default().fg(Color::Gray)))
            .borders(Borders::ALL))
        .legend_position(Some(LegendPosition::TopRight))
        .x_axis(Axis::default()
            .style(Style::default().fg(Color::Gray))
            .bounds([view_start_sec, view_end_sec]))
        .y_axis(Axis::default()
            .title("ms")
            .style(Style::default().fg(Color::Gray))
            .bounds([0.0, 100.0])
            .labels(vec![
                Span::styled("0", Style::default()),
                Span::styled("50", Style::default()),
                Span::styled("100", Style::default().fg(Color::Red))
            ]));

    f.render_widget(chart, area);
}

fn draw_host_stats(f: &mut Frame, area: Rect, stats: &HostStats, label: &str, app: &App) {
    let loss_percent = if stats.total_count > 0 {
        (stats.loss_count as f64 / stats.total_count as f64) * 100.0
    } else {
        0.0
    };

    let (p25, p75, p95) = calculate_percentiles(&stats.all_latencies);
    
    let is_gateway = label == "GATEWAY";

    let grade = if is_gateway {
        if loss_percent >= 1.0 || p95 >= 50.0 { "F" }
        else if loss_percent > 0.0 || p95 >= 20.0 { "C" }
        else if p95 >= 10.0 { "B" }
        else if p95 >= 2.0 { "A" }
        else { "S" }
    } else {
        if loss_percent >= 5.0 || p95 >= 120.0 { "F" } 
        else if loss_percent >= 2.0 || p95 >= 60.0 { "C" } 
        else if loss_percent >= 0.5 || p95 >= 30.0 { "B" } 
        else if loss_percent > 0.0  || p95 >= 10.0 { "A" } 
        else { "S" }
    };

    let grade_color = match grade {
        "S" | "A" => Color::Green,
        "B" => Color::Cyan,
        "C" => Color::Yellow,
        _ => Color::Red,
    };

    let limit_str = if let Some(max) = app.max_duration {
        format!("/{:02}:{:02}", max.as_secs()/60, max.as_secs()%60)
    } else {
        String::new()
    };
    
    let runtime_str = format!("{:02}:{:02}{}", (app.recorded_duration as u64)/60, (app.recorded_duration as u64)%60, limit_str);

    let spans = vec![
        Span::raw(" Loss: "),
        Span::styled(format!("{:.1}% ", loss_percent), Style::default().fg(if stats.loss_count == 0 { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD)),
        Span::raw("| P(25/75/95): "),
        Span::styled(format!("{:.0}/{:.0}/{:.0}ms ", p25, p75, p95), Style::default().fg(Color::Cyan)),
        Span::raw("| Spikes >30ms: "),
        Span::styled(format!("{} ", stats.spikes_minor), Style::default().fg(if stats.spikes_minor == 0 { Color::Green } else { Color::Yellow })),
        Span::raw("| >100ms: "),
        Span::styled(format!("{} ", stats.spikes_major), Style::default().fg(if stats.spikes_major == 0 { Color::Green } else { Color::Red })),
        Span::raw("| Grade: "),
        Span::styled(grade, Style::default().fg(grade_color).add_modifier(Modifier::BOLD)),
    ];

    let title = if label == "TARGET" {
        format!(" Stats ({}) - Time: {} ", label, runtime_str)
    } else {
        format!(" Stats ({}) ", label)
    };

    let p = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));
    
    f.render_widget(p, area);
}

fn draw_footer(f: &mut Frame, area: Rect, _app: &App) {
    let p = Paragraph::new(" [Q] Quit | [SPACE] Pause | [+/-] Zoom | [←/→] History ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn calculate_percentiles(latencies: &Vec<f64>) -> (f64, f64, f64) {
    let len = latencies.len();

    if len > 10 {
        let sample_limit = 10_000;
        let start_index = if len > sample_limit { len - sample_limit } else { 0 };
        
        let mut sorted = latencies[start_index..].to_vec();
        
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let sorted_len = sorted.len() as f64;
        (
            sorted[(sorted_len * 0.25) as usize],
            sorted[(sorted_len * 0.75) as usize],
            sorted[(sorted_len * 0.95) as usize]
        )
    } else {
        (0.0, 0.0, 0.0)
    }
}