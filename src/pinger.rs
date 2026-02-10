use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};

#[derive(Debug, Clone, PartialEq)]
pub enum SourceType {
    Internet,
    Gateway,
}

#[derive(Debug)]
pub struct PingUpdate {
    pub source: SourceType,
    pub latency: f64,
}

pub async fn run_pinger(
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