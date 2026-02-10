use clap::{Parser, ValueEnum};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub target: Option<String>,

    #[arg(short, long, value_enum, default_value_t = PingMode::Gaming)]
    pub mode: PingMode,

    #[arg(short, long)]
    pub duration: Option<String>,

    #[arg(short, long)]
    pub interval: Option<String>,

    #[arg(long, default_value_t = false)]
    pub no_gateway: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum PingMode {
    Gaming,
    Standard,
    Monitor,
}

pub fn parse_duration_string(s: &str) -> Option<Duration> {
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
    } else {
        None
    }
}