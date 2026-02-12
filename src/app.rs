use crate::pinger::SourceType;
use chrono::{DateTime, Local};
use crossterm::event::KeyCode;
use serde::Serialize;
use std::time::Instant;

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

    pub p25: f64,
    pub p75: f64,
    pub p99: f64,

    pub last_recalc: Instant,
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

            p25: 0.0,
            p75: 0.0,
            p99: 0.0,

            last_recalc: Instant::now(),
        }
    }

    fn update(&mut self, latency_opt: Option<f64>, time_val: f64) -> PingRecord {
        self.total_count += 1;
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string();
        
        match latency_opt {
            None => {
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

            Some(latency) => {
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

                let should_recalc = self.all_latencies.len() < 50 
                    || self.last_recalc.elapsed().as_secs_f64() >= 1.0;

                if should_recalc {
                    self.recalculate_percentiles();
                    self.last_recalc = Instant::now();
                }

                PingRecord {
                    timestamp,
                    target_type: "Unknown".to_string(),
                    target_ip: self.display_name.clone(),
                    latency_ms: Some(latency),
                    status: "OK".to_string(),
                }
            }
        }
    }

    fn recalculate_percentiles(&mut self) {
        let len = self.all_latencies.len();
        if len > 10 {
            let sample_limit = 100_000;
            let start_index = if len > sample_limit {
                len - sample_limit
            } else {
                0
            };

            let mut sorted = self.all_latencies[start_index..].to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let sorted_len = sorted.len() as f64;
            let max_idx = sorted_len - 1.0;

            self.p25 = sorted[(max_idx * 0.25).round() as usize];
            self.p75 = sorted[(max_idx * 0.75).round() as usize];
            self.p99 = sorted[(max_idx * 0.99).round() as usize];
        }
    }

    pub fn calculate_grade(&self, is_gateway: bool) -> &'static str {
        let loss_percent = if self.total_count > 0 {
            (self.loss_count as f64 / self.total_count as f64) * 100.0
        } else {
            0.0
        };

        if is_gateway {
            if loss_percent >= 1.0 || self.p99 >= 50.0 { "F" }
            else if loss_percent > 0.0 || self.p99 >= 25.0 { "C" }
            else if self.p99 >= 10.0 { "B" }
            else if self.p99 >= 5.0 { "A" }
            else { "S" }
        } else {
            if loss_percent >= 5.0 || self.p99 >= 150.0 { "F" } 
            else if loss_percent >= 2.0 || self.p99 >= 100.0 { "C" } 
            else if loss_percent >= 0.5 || self.p99 >= 70.0 { "B" } 
            else if loss_percent > 0.0  || self.p99 >= 40.0 { "A" } 
            else { "S" }
        }
    }
}

pub struct App {
    pub net_stats: HostStats,
    pub gw_stats: Option<HostStats>,

    pub start_time: DateTime<Local>,
    pub recorded_duration: f64,

    pub configured_interval: u64,

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
        interval_ms_float: f64,
        configured_interval: u64,
        max_duration: Option<std::time::Duration>,
    ) -> Self {
        Self {
            net_stats: HostStats::new(target_host),
            gw_stats: gateway_host.map(HostStats::new),

            start_time: Local::now(),
            recorded_duration: 0.0,

            configured_interval,

            zoom_window_seconds: if interval_ms_float <= 200.0 {
                60.0
            } else {
                300.0
            },
            scroll_offset_seconds: 0.0,

            is_paused: false,
            should_quit: false,
            is_finished: false,
            max_duration,
        }
    }

    pub fn on_ping(&mut self, source: SourceType, latency: Option<f64>) -> Option<PingRecord> {
        if self.is_paused || self.is_finished {
            return None;
        }

        let now = Local::now();
        let duration_since_start = now.signed_duration_since(self.start_time);
        
        let time_val = duration_since_start.num_milliseconds() as f64 / 1000.0;
        
        if time_val > self.recorded_duration {
            self.recorded_duration = time_val;
        }
        
        if let Some(max) = self.max_duration {
            if self.recorded_duration >= max.as_secs_f64() {
                self.is_finished = true;
                return None;
            }
        }
        
        let record = match source {
            SourceType::Target => {
                let mut r = self.net_stats.update(latency, time_val);
                r.target_type = "Target".to_string();
                Some(r)
            },
            
            SourceType::Gateway => {
                if let Some(gw) = &mut self.gw_stats {
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
            }

            KeyCode::Char('+') | KeyCode::Up => {
                if self.zoom_window_seconds > 10.0 {
                    self.zoom_window_seconds -= 10.0;
                }
            }

            KeyCode::Char('-') | KeyCode::Down => {
                self.zoom_window_seconds += 10.0;
            }

            KeyCode::Left => {
                if self.scroll_offset_seconds < self.recorded_duration {
                    self.scroll_offset_seconds += 10.0;
                }
            }

            KeyCode::Right => {
                self.scroll_offset_seconds -= 10.0;
                if self.scroll_offset_seconds < 0.0 {
                    self.scroll_offset_seconds = 0.0;
                }
            }
            _ => {}
        }
    }
}