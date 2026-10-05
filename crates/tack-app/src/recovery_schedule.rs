//! Dirty-only deadlines. Idle clean or already recovered boards have no timer.
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct RecoverySchedule {
    observed: u64,
    first: Option<Instant>,
    due: Option<Instant>,
    pub saved_generation: Option<u64>,
}
impl RecoverySchedule {
    pub fn observe(&mut self, generation: u64, dirty: bool, now: Instant) {
        if !dirty {
            self.first = None;
            self.due = None;
            self.observed = generation;
            return;
        }
        if generation != self.observed {
            self.observed = generation;
            let first = *self.first.get_or_insert(now);
            self.due = Some((now + Duration::from_secs(5)).min(first + Duration::from_secs(30)));
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.due
    }
    pub fn ready(&self, now: Instant) -> bool {
        self.due.is_some_and(|due| now >= due)
    }
    pub fn started(&mut self) {
        self.due = None;
        self.first = None;
    }
    pub fn restored(&mut self, generation: u64) {
        self.observed = generation;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clean_idle_and_recovered_idle_have_no_timer_continuous_edits_are_bounded() {
        let now = Instant::now();
        let mut s = RecoverySchedule::default();
        s.observe(0, false, now);
        assert_eq!(s.deadline(), None);
        s.observe(1, true, now);
        assert!(!s.ready(now + Duration::from_secs(4)));
        for i in 2..=20 {
            s.observe(i, true, now + Duration::from_secs(i));
        }
        assert!(
            s.deadline()
                .is_some_and(|d| d <= now + Duration::from_secs(30))
        );
        s.started();
        s.observe(20, true, now + Duration::from_secs(40));
        assert_eq!(s.deadline(), None);
        s.observe(21, true, now + Duration::from_secs(41));
        assert!(s.deadline().is_some());
        s.observe(21, false, now);
        assert_eq!(s.deadline(), None);
    }
}
