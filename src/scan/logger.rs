use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::SharedRunState;

use super::{ControlRx, ControlTx};

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

pub struct Logger {
    interval: Duration,
    start: Instant,
    stats: LoggerStats,
    run_state: SharedRunState,
    control_rx: ControlRx,
}

impl Logger {
    pub fn new(run_state: SharedRunState) -> (Self, LoggerStats, ControlTx) {
        let stats = Arc::new(Mutex::new(Stats::new()));

        let (control_tx, control_rx) = channel();

        let logger = Logger {
            interval: Duration::new(1, 0),
            start: Instant::now(),
            stats: stats.clone(),
            run_state,
            control_rx,
        };

        (logger, stats, control_tx)
    }

    pub fn format_time(duration: &Duration) -> String {
        let seconds = duration.as_secs();
        let minutes: u64 = seconds / 60;
        let hours: u64 = minutes / 60;
        let days: u64 = hours / 24;

        let mut time = String::new();
        if days >= 1 {
            time = time + &format!("{}d ", days);
        };
        if hours >= 1 {
            time = time + &format!("{}h ", hours % 24);
        };
        if minutes >= 1 {
            time = time + &format!("{}m ", minutes % 60);
        };
        time = time + &format!("{}s", seconds % 60);

        time
    }

    pub fn log(self) {
        let mut paused_offset = Duration::new(0, 0);
        loop {
            if *self.run_state.paused.lock().unwrap() {
                let started = Instant::now();
                self.run_state.act_state();
                paused_offset += Instant::now() - started;
            }
            if let Ok(_) = self.control_rx.try_recv() {
                return;
            }
            let timespan = Instant::now() - self.start - paused_offset;

            {
                let data = self.stats.lock().unwrap();
                let send_kbps = (data.sent as f64 / (10u64.pow(3) as f64)) / timespan.as_secs_f64();
                let recv_ps = data.received as f64 / timespan.as_secs_f64();
                let time = Self::format_time(&timespan);
                let remaining = Self::format_time(&Duration::new(
                    (((timespan.as_secs() as f64 + 1.0) / ((data.sent as f64) + 1.0)) * ((2 as f64).powf(32.0) - data.sent as f64)) as u64,
                    0,
                ));
                println!(
                    "{}; Sent: {:.2} at {:.2} Kp/s; Received: {:.2} at {:.2} p/s; left: {}",
                    time, data.sent, send_kbps, data.received, recv_ps, remaining
                );
            }
            std::thread::sleep(self.interval);
        }
    }
}
