//! "Monitor for activity" / "monitor for silence" (Yakuake session options). Pure state machine; the caller supplies the clock.

use std::time::{Duration, Instant};

pub const DEFAULT_SILENCE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorNotification {
    /// Output arrived after the terminal was last acknowledged.
    ActivityDetected,
    /// No output for the silence timeout after some output.
    SilenceDetected,
}

#[derive(Debug, Clone)]
pub struct ActivityMonitor {
    silence_timeout: Duration,
    watching_activity: bool,
    watching_silence: bool,
    last_output: Option<Instant>,
    activity_pending: bool,
    activity_reported: bool,
    silence_armed: bool,
}

impl ActivityMonitor {
    pub fn new(silence_timeout: Duration) -> Self {
        Self {
            silence_timeout,
            watching_activity: false,
            watching_silence: false,
            last_output: None,
            activity_pending: false,
            activity_reported: false,
            silence_armed: false,
        }
    }

    pub fn is_watching_activity(&self) -> bool {
        self.watching_activity
    }

    pub fn is_watching_silence(&self) -> bool {
        self.watching_silence
    }

    pub fn set_watching_activity(&mut self, enabled: bool) {
        self.watching_activity = enabled;
        self.activity_pending = false;
        self.activity_reported = false;
    }

    pub fn set_watching_silence(&mut self, enabled: bool) {
        self.watching_silence = enabled;
        self.silence_armed = false;
    }

    pub fn record_output(&mut self, now: Instant) {
        self.last_output = Some(now);
        if self.watching_activity && !self.activity_reported {
            self.activity_pending = true;
        }
        if self.watching_silence {
            self.silence_armed = true;
        }
    }

    /// The user looked at the terminal: the next output counts as new activity again.
    pub fn acknowledge(&mut self) {
        self.activity_pending = false;
        self.activity_reported = false;
    }

    /// Returns at most one notification per call; each fires once until re-armed by new output or `acknowledge`.
    pub fn poll(&mut self, now: Instant) -> Option<MonitorNotification> {
        if self.activity_pending {
            self.activity_pending = false;
            self.activity_reported = true;
            return Some(MonitorNotification::ActivityDetected);
        }
        let last_output = self.last_output?;
        if self.silence_armed && now.saturating_duration_since(last_output) >= self.silence_timeout {
            self.silence_armed = false;
            return Some(MonitorNotification::SilenceDetected);
        }
        None
    }
}

impl Default for ActivityMonitor {
    fn default() -> Self {
        Self::new(DEFAULT_SILENCE_TIMEOUT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    fn monitor(activity: bool, silence: bool) -> (ActivityMonitor, Instant) {
        let mut monitor = ActivityMonitor::new(10 * SECOND);
        monitor.set_watching_activity(activity);
        monitor.set_watching_silence(silence);
        (monitor, Instant::now())
    }

    #[test]
    fn given_activity_monitoring_when_output_arrives_then_activity_is_reported_once() {
        let (mut monitor, start) = monitor(true, false);
        monitor.record_output(start);
        assert_eq!(monitor.poll(start), Some(MonitorNotification::ActivityDetected));
        monitor.record_output(start + SECOND);
        assert_eq!(monitor.poll(start + SECOND), None);
    }

    #[test]
    fn given_reported_activity_when_acknowledged_then_next_output_reports_again() {
        let (mut monitor, start) = monitor(true, false);
        monitor.record_output(start);
        monitor.poll(start);
        monitor.acknowledge();
        monitor.record_output(start + SECOND);
        assert_eq!(monitor.poll(start + SECOND), Some(MonitorNotification::ActivityDetected));
    }

    #[test]
    fn given_activity_monitoring_disabled_when_output_arrives_then_nothing_is_reported() {
        let (mut monitor, start) = monitor(false, false);
        monitor.record_output(start);
        assert_eq!(monitor.poll(start + 60 * SECOND), None);
    }

    #[test]
    fn given_silence_monitoring_when_output_stops_for_the_timeout_then_silence_is_reported_once() {
        let (mut monitor, start) = monitor(false, true);
        monitor.record_output(start);
        assert_eq!(monitor.poll(start + 9 * SECOND), None);
        assert_eq!(monitor.poll(start + 10 * SECOND), Some(MonitorNotification::SilenceDetected));
        assert_eq!(monitor.poll(start + 30 * SECOND), None);
    }

    #[test]
    fn given_silence_monitoring_when_output_keeps_coming_then_silence_timer_restarts() {
        let (mut monitor, start) = monitor(false, true);
        monitor.record_output(start);
        monitor.record_output(start + 8 * SECOND);
        assert_eq!(monitor.poll(start + 12 * SECOND), None);
        assert_eq!(monitor.poll(start + 18 * SECOND), Some(MonitorNotification::SilenceDetected));
    }

    #[test]
    fn given_silence_reported_when_new_output_arrives_then_silence_can_fire_again() {
        let (mut monitor, start) = monitor(false, true);
        monitor.record_output(start);
        monitor.poll(start + 10 * SECOND);
        monitor.record_output(start + 11 * SECOND);
        assert_eq!(monitor.poll(start + 21 * SECOND), Some(MonitorNotification::SilenceDetected));
    }

    #[test]
    fn given_no_output_ever_when_polling_then_silence_is_not_reported() {
        let (mut monitor, start) = monitor(false, true);
        assert_eq!(monitor.poll(start + 100 * SECOND), None);
    }

    #[test]
    fn given_both_pending_when_polling_then_activity_comes_first() {
        let (mut monitor, start) = monitor(true, true);
        monitor.record_output(start);
        assert_eq!(monitor.poll(start + 20 * SECOND), Some(MonitorNotification::ActivityDetected));
        assert_eq!(monitor.poll(start + 20 * SECOND), Some(MonitorNotification::SilenceDetected));
    }

    #[test]
    fn given_silence_monitoring_when_disabled_then_armed_silence_is_dropped() {
        let (mut monitor, start) = monitor(false, true);
        monitor.record_output(start);
        monitor.set_watching_silence(false);
        assert_eq!(monitor.poll(start + 20 * SECOND), None);
        assert!(!monitor.is_watching_silence());
    }
}
