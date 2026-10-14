use crate::command::worker_protocol::ControlState;
use std::time::{Duration, Instant};

#[derive(Debug)]
enum ClockState {
    Running(Instant),
    Paused,
    Finished,
}

/// One monotonic clock for the entire job, not one clock per processing phase.
#[derive(Debug)]
pub(crate) struct JobClock {
    accumulated: Duration,
    state: ClockState,
}

impl JobClock {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            accumulated: Duration::ZERO,
            state: ClockState::Running(now),
        }
    }

    pub(crate) fn restore(accumulated: Duration, now: Instant) -> Self {
        Self {
            accumulated,
            ..Self::new(now)
        }
    }

    pub(crate) fn elapsed(&self, now: Instant) -> Duration {
        match self.state {
            ClockState::Running(since) => self.accumulated + now.saturating_duration_since(since),
            ClockState::Paused | ClockState::Finished => self.accumulated,
        }
    }

    pub(crate) fn set_state(&mut self, state: ControlState, now: Instant) {
        if matches!(self.state, ClockState::Finished) {
            return;
        }
        self.accumulated = self.elapsed(now);
        self.state = match state {
            ControlState::Running => ClockState::Running(now),
            ControlState::Paused => ClockState::Paused,
            ControlState::Stopped => ClockState::Finished,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::worker_protocol::ControlState;
    use std::time::{Duration, Instant};

    #[test]
    fn clock_excludes_every_pause_and_freezes_on_stop() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut clock = JobClock::new(start);
        clock.set_state(ControlState::Paused, at(3));
        assert_eq!(clock.elapsed(at(30)), Duration::from_secs(3));
        clock.set_state(ControlState::Paused, at(40));
        clock.set_state(ControlState::Running, at(50));
        clock.set_state(ControlState::Running, at(51));
        clock.set_state(ControlState::Paused, at(55));
        clock.set_state(ControlState::Running, at(100));
        clock.set_state(ControlState::Stopped, at(104));
        clock.set_state(ControlState::Running, at(200));
        assert_eq!(clock.elapsed(at(300)), Duration::from_secs(12));
    }

    #[test]
    fn restarted_job_resumes_its_checkpoint_without_charging_downtime() {
        let restart = Instant::now();
        let mut clock = JobClock::restore(Duration::from_secs(17), restart);
        assert_eq!(
            clock.elapsed(restart + Duration::from_secs(3)),
            Duration::from_secs(20)
        );
        clock.set_state(ControlState::Paused, restart + Duration::from_secs(3));
        assert_eq!(
            clock.elapsed(restart + Duration::from_secs(300)),
            Duration::from_secs(20)
        );
    }

    #[test]
    fn independent_clocks_cover_transfer_probe_encode_and_reconnect() {
        let start = Instant::now();
        let mut search = JobClock::new(start);
        let encode = JobClock::new(start);
        search.set_state(ControlState::Paused, start + Duration::from_secs(2));
        assert_eq!(
            search.elapsed(start + Duration::from_secs(20)),
            Duration::from_secs(2)
        );
        assert_eq!(
            encode.elapsed(start + Duration::from_secs(20)),
            Duration::from_secs(20)
        );
        // Reconnection reads the existing job clock; it does not create a phase clock.
        search.set_state(ControlState::Running, start + Duration::from_secs(20));
        assert_eq!(
            search.elapsed(start + Duration::from_secs(25)),
            Duration::from_secs(7)
        );
    }
}
