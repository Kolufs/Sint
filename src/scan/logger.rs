use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flume::{Receiver, Sender};

#[derive(Debug, Clone)]
pub struct Stats {
    pub sent: u64,
    pub received: u64,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            sent: 0,
            received: 0,
        }
    }
}

pub type LoggerStats = Arc<Mutex<Stats>>;

pub enum LoggerControlMessage {
    Die,
    Pause,
}

pub struct Logger {
    interval: Duration,
    start: Instant,
    stats: LoggerStats,
    rendezvous_control_rx: Receiver<LoggerControlMessage>,
}

pub type LoggerHandler = Sender<LoggerControlMessage>;

impl Logger {
    pub fn new() -> (Self, LoggerStats, LoggerHandler) {
        let stats = Arc::new(Mutex::new(Stats::new()));

        let (rendezvous_control_tx, rendezvous_control_rx) = flume::bounded(0);

        let logger = Logger {
            interval: Duration::new(1, 0),
            start: Instant::now(),
            stats: stats.clone(),
            rendezvous_control_rx,
        };

        (logger, stats, rendezvous_control_tx)
    }

    pub fn log(self) {
        let mut last_log = Instant::now();
        loop {
            if let None = self.interval.checked_sub(Instant::now() - last_log) {
                let timespan = Instant::now() - self.start;
                let data = self.stats.lock().unwrap();
                let hitrate = (data.received as f64 / data.sent as f64) * 100.0;
                let send_kbps =
                    (data.sent as f64 / (10u64.pow(3) as f64)) / timespan.as_secs_f64();
                let recv_ps = data.received as f64 / timespan.as_secs_f64();

                println!(
                    "{:#?}: Hitrate: {}%; Sent: {:.2} at {} Kp/s; Received: {} at {:.2} p/s",
                    timespan, hitrate, data.sent, send_kbps, data.received, recv_ps
                );
            }

            last_log = Instant::now();

            if let Ok(message) = self.rendezvous_control_rx.try_recv() {
                match message {
                    LoggerControlMessage::Die => todo!(),
                    LoggerControlMessage::Pause => todo!(),
                }
            }

            std::thread::sleep(self.interval);
        }
    }
}
