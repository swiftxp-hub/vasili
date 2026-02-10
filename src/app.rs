use chrono::{DateTime, Local};
use crossterm::event::KeyCode;
use serde::Serialize;
use crate::pinger::SourceType;

#[derive(Debug, Serialize, Clone)]
pub struct PingRecord {
    pub timestamp: String,
    pub target_type: String,
    pub target_ip: String,
    pub latency_ms: Option<f64>,
    pub status: String,
}

pub struct HostStats {
    pub display_name: String,
    pub points: Vec<(f64, f64)>,
    pub jitter_points: Vec<(f64, f64)>,
    pub loss_points: Vec<(f64, f64)>,
    pub all_latencies: Vec<f64>,
    
    pub last_latency: f64,
    pub current_jitter: f64,
    pub total_count: u64,
    pub loss_count: u64,
    pub spikes_minor: u64,
    pub spikes_major: u64,
}

impl HostStats {
    fn new(display_name: String) -> Self {
        Self {
            display_name,
            points: Vec::new(),
            jitter_points: Vec::new(),
            loss_points: Vec::new(),
            all_latencies: Vec::new(),
            last_latency: 0.0,
            current_jitter: 0.0,
            total_count: 0,
            loss_count: 0,
            spikes_minor: 0,
            spikes_major: 0,
        }
    }

    fn update(&mut self, latency: f64, time_val: f64) -> PingRecord {
        self.total_count += 1;
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();
        
        if latency < 0.0 {
            self.loss_count += 1;
            self.spikes_major += 1;
            self.loss_points.push((time_val, 100.0));
            
            return PingRecord {
                timestamp,
                target_type: "Unknown".to_string(),
                target_ip: self.display_name.clone(),
                latency_ms: None,
                status: "TIMEOUT".to_string(),
            };
        }

        let jitter = if self.last_latency == 0.0 { 
            0.0 
        } else { 
            (latency - self.last_latency).abs() 
        };

        self.last_latency = latency;
        self.current_jitter = jitter;
        self.all_latencies.push(latency);

        if latency >= 100.0 {
            self.spikes_major += 1;
        } else if latency >= 30.0 {
            self.spikes_minor += 1;
        }

        self.points.push((time_val, latency));
        self.jitter_points.push((time_val, jitter));

        PingRecord {
            timestamp,
            target_type: "Unknown".to_string(),
            target_ip: self.display_name.clone(),
            latency_ms: Some(latency),
            status: "OK".to_string(),
        }
    }
}

pub struct App {
    pub net_stats: HostStats,
    pub gw_stats: Option<HostStats>,

    pub start_time: DateTime<Local>,
    pub recorded_duration: f64,
    pub x_counter: f64,
    pub time_factor: f64,
    
    pub zoom_window_seconds: f64,
    pub scroll_offset_seconds: f64,

    pub is_paused: bool,
    pub should_quit: bool,
    pub is_finished: bool,
    pub max_duration: Option<std::time::Duration>,
}

impl App {
    pub fn new(
        target_host: String, 
        gateway_host: Option<String>, 
        interval_ms: u64,
        max_duration: Option<std::time::Duration>
    ) -> Self {
        Self {
            net_stats: HostStats::new(target_host),
            gw_stats: gateway_host.map(HostStats::new),
            
            start_time: Local::now(),
            recorded_duration: 0.0,
            x_counter: 0.0,
            time_factor: interval_ms as f64 / 1000.0,
            
            zoom_window_seconds: if interval_ms <= 200 { 60.0 } else { 300.0 },
            scroll_offset_seconds: 0.0,
            
            is_paused: false,
            should_quit: false,
            is_finished: false,
            max_duration,
        }
    }

    pub fn on_ping(&mut self, source: SourceType, latency: f64) -> Option<PingRecord> {
        if self.is_paused || self.is_finished {
            return None;
        }

        if let Some(max) = self.max_duration {
            if self.recorded_duration >= max.as_secs_f64() {
                self.is_finished = true;
                return None;
            }
        }

        let time_val = self.x_counter * self.time_factor;
        
        let record = match source {
            SourceType::Target => {
                self.x_counter += 1.0;
                self.recorded_duration += self.time_factor;
                let mut r = self.net_stats.update(latency, time_val);
                r.target_type = "Target".to_string();
                Some(r)
            },
            
            SourceType::Gateway => {
                if let Some(gw) = &mut self.gw_stats {
                    self.x_counter += 1.0;
                    self.recorded_duration += self.time_factor;
                    
                    let mut r = gw.update(latency, time_val);
                    r.target_type = "Gateway".to_string();
                    Some(r)
                } else {
                    None
                }
            }
        };

        record
    }

    pub fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char(' ') => {
                if !self.is_finished {
                    self.is_paused = !self.is_paused;
                }
            },
            KeyCode::Char('+') | KeyCode::Up => {
                if self.zoom_window_seconds > 10.0 {
                    self.zoom_window_seconds -= 10.0;
                }
            },
            KeyCode::Char('-') | KeyCode::Down => {
                self.zoom_window_seconds += 10.0;
            },
            KeyCode::Left => {
                let current_time = self.x_counter * self.time_factor;
                if self.scroll_offset_seconds < current_time {
                    self.scroll_offset_seconds += 10.0;
                }
            },
            KeyCode::Right => {
                self.scroll_offset_seconds -= 10.0;
                if self.scroll_offset_seconds < 0.0 {
                    self.scroll_offset_seconds = 0.0;
                }
            },
            _ => {}
        }
    }
}