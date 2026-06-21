use std::thread::sleep;
use std::time::Duration;

/// Политика повторов с экспоненциальной задержкой.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// Максимум попыток, включая первую.
    pub attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl Backoff {
    /// Задержка перед повтором номер `retry_no` (нумерация с 1): база × 2^(n−1), с потолком.
    fn delay_before(&self, retry_no: u32) -> Duration {
        self.base_delay
            .saturating_mul(2_u32.saturating_pow(retry_no - 1))
            .min(self.max_delay)
    }
}

/// Выполняет `op` до `policy.attempts` раз. Повторяет только те ошибки,
/// для которых `should_retry` вернул true; последняя ошибка отдаётся наружу.
pub fn retry<T, E>(
    policy: &Backoff,
    mut should_retry: impl FnMut(&E) -> bool,
    mut op: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let mut retries = 0;
    loop {
        match op() {
            Ok(value) => return Ok(value),
            Err(err) if retries + 1 < policy.attempts && should_retry(&err) => {
                retries += 1;
                sleep(policy.delay_before(retries));
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> Backoff {
        Backoff {
            attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        }
    }

    #[test]
    fn succeeds_after_transient_failures() {
        let mut calls = 0;
        let result = retry(
            &fast(),
            |_: &&str| true,
            || {
                calls += 1;
                if calls < 3 {
                    Err("временная")
                } else {
                    Ok(calls)
                }
            },
        );
        assert_eq!(result, Ok(3));
    }

    #[test]
    fn gives_up_after_all_attempts() {
        let mut calls = 0;
        let result: Result<(), _> = retry(
            &fast(),
            |_| true,
            || {
                calls += 1;
                Err("всегда падаю")
            },
        );
        assert_eq!(result, Err("всегда падаю"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn does_not_retry_permanent_errors() {
        let mut calls = 0;
        let result: Result<(), _> = retry(
            &fast(),
            |_| false,
            || {
                calls += 1;
                Err("постоянная")
            },
        );
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }
}
