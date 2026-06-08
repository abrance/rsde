use std::{thread, time::Duration};

use anyhow::Result;
use tracing::{error, info, warn};

const TRANSPORT_RETRY_DELAYS_MS: [u64; 2] = [5, 10];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetryDiagnostic {
    pub attempts: usize,
    pub retries: usize,
    pub exhausted: bool,
}

pub(crate) struct RetryOutcome<T> {
    pub result: Result<T>,
    pub diagnostic: RetryDiagnostic,
}

pub(crate) fn transport_retry_delay(attempt: usize) -> Option<Duration> {
    TRANSPORT_RETRY_DELAYS_MS
        .get(attempt)
        .copied()
        .map(Duration::from_millis)
}

pub(crate) fn transport_max_attempts() -> usize {
    TRANSPORT_RETRY_DELAYS_MS.len() + 1
}

pub(crate) fn retry_blocking_transport_with_diagnostic<T>(
    operation_name: &str,
    mut operation: impl FnMut() -> Result<T>,
) -> RetryOutcome<T> {
    let mut attempt = 0;

    loop {
        let current_attempt = attempt + 1;

        match operation() {
            Ok(value) => {
                let diagnostic = RetryDiagnostic {
                    attempts: current_attempt,
                    retries: attempt,
                    exhausted: false,
                };

                if diagnostic.retries > 0 {
                    info!(
                        operation = operation_name,
                        retries = diagnostic.retries,
                        recovered_on_attempt = diagnostic.attempts,
                        max_attempts = transport_max_attempts(),
                        "transport operation recovered after retry"
                    );
                }

                return RetryOutcome {
                    result: Ok(value),
                    diagnostic,
                };
            }
            Err(error) => match transport_retry_delay(attempt) {
                Some(delay) => {
                    warn!(
                        operation = operation_name,
                        failed_attempt = current_attempt,
                        next_attempt = current_attempt + 1,
                        max_attempts = transport_max_attempts(),
                        delay_ms = delay.as_millis() as u64,
                        error = %error,
                        "transport operation failed; retrying"
                    );
                    attempt += 1;
                    thread::sleep(delay);
                }
                None => {
                    let diagnostic = RetryDiagnostic {
                        attempts: current_attempt,
                        retries: attempt,
                        exhausted: true,
                    };

                    error!(
                        operation = operation_name,
                        failed_attempt = diagnostic.attempts,
                        max_attempts = transport_max_attempts(),
                        error = %error,
                        "transport operation failed; retry budget exhausted"
                    );

                    return RetryOutcome {
                        result: Err(error),
                        diagnostic,
                    };
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::anyhow;

    use super::retry_blocking_transport_with_diagnostic;

    #[test]
    fn retry_helper_reports_attempts_after_recovery() {
        let attempts = Arc::new(Mutex::new(0usize));
        let attempts_for_op = Arc::clone(&attempts);

        let outcome = retry_blocking_transport_with_diagnostic("test-op", move || {
            let mut guard = attempts_for_op.lock().expect("attempts lock");
            *guard += 1;
            if *guard == 1 {
                Err(anyhow!("temporary failure"))
            } else {
                Ok("ok")
            }
        });

        assert_eq!(outcome.result.expect("retry should recover"), "ok");
        assert_eq!(outcome.diagnostic.attempts, 2);
        assert_eq!(outcome.diagnostic.retries, 1);
        assert!(!outcome.diagnostic.exhausted);
    }

    #[test]
    fn retry_helper_reports_exhaustion_after_last_attempt() {
        let attempts = Arc::new(Mutex::new(0usize));
        let attempts_for_op = Arc::clone(&attempts);

        let outcome: super::RetryOutcome<&'static str> =
            retry_blocking_transport_with_diagnostic("test-op", move || {
                let mut guard = attempts_for_op.lock().expect("attempts lock");
                *guard += 1;
                Err(anyhow!(format!("temporary failure {}", *guard)))
            });

        assert!(outcome.result.is_err());
        assert_eq!(*attempts.lock().expect("attempts lock"), 3);
        assert_eq!(outcome.diagnostic.attempts, 3);
        assert_eq!(outcome.diagnostic.retries, 2);
        assert!(outcome.diagnostic.exhausted);
    }
}
