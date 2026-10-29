//! Tests of the icon retry loop (`icon_retry.rs`) on a fake host: no icon, no bus, no real pauses.

use super::*;

const ERROR_TEXT: &str = "no status notifier watcher";

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

mod first_attempt_succeeds {
    use super::*;

    #[test]
    fn first_attempt_ok_attaches_without_wait_or_log() {
        // given
        let mut host = FakeHost::ok_on_attempt(1);

        // when
        let outcome = raise_with_retry(&mut host, RetryPlan::LINUX);

        // then
        assert_eq!(outcome, Raise::Attached { attempts: 1 });
        assert_eq!(host.attempts, 1);
        assert!(host.waits.is_empty(), "waited: {:?}", host.waits);
        assert!(host.logs.is_empty(), "logged: {:?}", host.logs);
    }
}

mod watcher_appears_late {
    use super::*;

    #[test]
    fn late_watcher_attaches_on_third_attempt_logging_once() {
        // given
        let mut host = FakeHost::ok_on_attempt(3);

        // when
        let outcome = raise_with_retry(&mut host, RetryPlan::LINUX);

        // then
        assert_eq!(outcome, Raise::Attached { attempts: 3 });
        assert_eq!(host.waits, vec![secs(1), secs(2)]);
        assert_eq!(host.logs.len(), 2, "logged: {:?}", host.logs);
        assert!(host.logs[0].contains("yet"), "first line: {}", host.logs[0]);
        assert!(
            host.logs[0].contains(ERROR_TEXT),
            "the error must be in the first line: {}",
            host.logs[0]
        );
        assert!(
            host.logs[1].contains("raised on attempt 3"),
            "second line: {}",
            host.logs[1]
        );
    }
}

mod watcher_never_appears {
    use super::*;

    #[test]
    fn never_ok_gives_up_after_max_attempts_with_two_lines() {
        // given
        let mut host = FakeHost::never_ok();

        // when
        let outcome = raise_with_retry(&mut host, RetryPlan::LINUX);

        // then
        assert_eq!(outcome, Raise::GaveUp { attempts: 25 });
        assert_eq!(host.attempts, 25);
        assert_eq!(host.waits.len(), 24, "one pause between each two attempts");
        assert_eq!(host.logs.len(), 2, "logged: {:?}", host.logs);
        assert!(host.logs[0].contains("yet"), "first line: {}", host.logs[0]);
        assert!(
            host.logs[1].contains("gave up"),
            "second line: {}",
            host.logs[1]
        );
    }
}

mod quit_requested {
    use super::*;

    #[test]
    fn quit_during_wait_stops_without_another_attempt() {
        // given
        let mut host = FakeHost::never_ok();
        host.quit_on_wait = Some((2, 5));

        // when
        let outcome = raise_with_retry(&mut host, RetryPlan::LINUX);

        // then
        assert_eq!(outcome, Raise::Quit(5));
        assert_eq!(host.attempts, 2, "no attempt after the quit request");
        assert_eq!(host.waits, vec![secs(1), secs(2)]);
    }
}

mod attempt_limit {
    use super::*;

    #[test]
    fn single_attempt_plan_never_waits() {
        // given
        let mut host = FakeHost::never_ok();
        let plan = plan_with_attempts(1);

        // when
        let outcome = raise_with_retry(&mut host, plan);

        // then
        assert_eq!(outcome, Raise::GaveUp { attempts: 1 });
        assert_eq!(host.attempts, 1);
        assert!(host.waits.is_empty(), "waited: {:?}", host.waits);
    }

    #[test]
    fn zero_attempt_plan_still_tries_once() {
        // given
        let mut host = FakeHost::never_ok();
        let plan = plan_with_attempts(0);

        // when
        let outcome = raise_with_retry(&mut host, plan);

        // then
        assert_eq!(outcome, Raise::GaveUp { attempts: 1 });
        assert_eq!(host.attempts, 1);
        assert!(host.waits.is_empty(), "waited: {:?}", host.waits);
    }
}

mod pause_schedule {
    use super::*;

    #[test]
    fn delay_doubles_then_stops_at_cap() {
        // given
        let plan = RetryPlan::LINUX;

        // when
        let delays: Vec<Duration> = (1..=7).map(|attempt| plan.delay(attempt)).collect();

        // then
        let expected = [1, 2, 4, 8, 16, 30, 30].map(secs).to_vec();
        assert_eq!(delays, expected);
    }

    #[test]
    fn delay_does_not_overflow() {
        // given
        let plan = RetryPlan::LINUX;

        // when
        let delay = plan.delay(u32::MAX);

        // then
        assert_eq!(delay, plan.cap);
    }

    #[test]
    fn linux_plan_waits_about_ten_minutes() {
        // given
        let plan = RetryPlan::LINUX;

        // when
        let total: Duration = (1..plan.max_attempts)
            .map(|attempt| plan.delay(attempt))
            .sum();

        // then
        assert_eq!(plan.max_attempts, 25);
        assert!(
            total >= secs(9 * 60) && total <= secs(11 * 60),
            "total wait {total:?}"
        );
    }
}

/// A host that records everything the loop does and fails or succeeds on a script.
struct FakeHost {
    /// The attempt (from 1) that succeeds; `None` means none does.
    ok_on: Option<u32>,
    /// The pause (from 1) that answers with an exit code, and the code.
    quit_on_wait: Option<(usize, i32)>,
    attempts: u32,
    waits: Vec<Duration>,
    logs: Vec<String>,
}

impl FakeHost {
    fn new(ok_on: Option<u32>) -> Self {
        FakeHost {
            ok_on,
            quit_on_wait: None,
            attempts: 0,
            waits: Vec::new(),
            logs: Vec::new(),
        }
    }

    fn ok_on_attempt(attempt: u32) -> Self {
        Self::new(Some(attempt))
    }

    fn never_ok() -> Self {
        Self::new(None)
    }
}

impl IconHost for FakeHost {
    fn try_attach(&mut self) -> Result<(), String> {
        self.attempts += 1;
        match self.ok_on {
            Some(n) if n == self.attempts => Ok(()),
            _ => Err(ERROR_TEXT.to_string()),
        }
    }

    fn wait(&mut self, pause: Duration) -> Option<i32> {
        self.waits.push(pause);
        match self.quit_on_wait {
            Some((n, code)) if n == self.waits.len() => Some(code),
            _ => None,
        }
    }

    fn log(&mut self, text: &str) {
        self.logs.push(text.to_string());
    }
}

/// The Linux schedule with another attempt limit.
fn plan_with_attempts(max_attempts: u32) -> RetryPlan {
    RetryPlan {
        max_attempts,
        ..RetryPlan::LINUX
    }
}
