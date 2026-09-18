use std::time::Instant;

pub(crate) trait Clock {
    fn now(&self) -> Instant;
}

struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[expect(dead_code)]
pub(crate) fn real_clock() -> impl Clock + Send + Sync + 'static {
    RealClock
}
