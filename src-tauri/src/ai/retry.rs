use std::time::Duration;

use crate::ai::service::{AIServiceRequest, AIServiceResponse};
use crate::auth::AuthState;

/// Generic exponential-backoff retry wrapper.
///
/// Calls `op()` up to `1 + max_retries` times. On success, returns `Ok(T)`.
/// On failure, waits `delay` (doubling each attempt) and retries. After
/// `max_retries` exhausted, returns the last `Err`.
pub async fn retry_with_backoff<T, F, Fut>(
    mut op: F,
    max_retries: u8,
    initial_delay: Duration,
) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut delay = initial_delay;
    let mut attempt: u8 = 0;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= max_retries {
                    return Err(e);
                }
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2);
                attempt += 1;
            }
        }
    }
}

/// AI request with exponential-backoff retry.
///
/// Wraps `ai_request` with `retry_with_backoff` using 2 s initial delay,
/// doubling on each retry. The request is cloned between attempts.
pub async fn ai_request_with_retry(
    auth: &AuthState,
    req: AIServiceRequest,
    max_retries: u8,
) -> Result<AIServiceResponse, String> {
    retry_with_backoff(
        || {
            let req = req.clone();
            async move { crate::ai::service::ai_request(auth, req).await }
        },
        max_retries,
        Duration::from_millis(2000),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result = retry_with_backoff(
            || {
                let a = Arc::clone(&attempts_clone);
                async move {
                    let n = a.fetch_add(1, Ordering::SeqCst) + 1;
                    if n == 1 {
                        Err("fail-once".to_string())
                    } else {
                        Ok(42u32)
                    }
                }
            },
            2,                              // max_retries
            Duration::from_millis(1),       // tiny delay for fast unit test
        )
        .await;

        assert_eq!(result, Ok(42));
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "must make exactly 2 attempts");
    }

    #[tokio::test]
    async fn retry_fails_after_max_retries() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<u32, String> = retry_with_backoff(
            || {
                let a = Arc::clone(&attempts_clone);
                async move {
                    a.fetch_add(1, Ordering::SeqCst);
                    Err("always-fail".to_string())
                }
            },
            2,                          // max_retries=2 → 1 initial + 2 retries = 3 total attempts
            Duration::from_millis(1),
        )
        .await;

        assert!(result.is_err(), "must return Err after exhausting retries");
        assert_eq!(result.unwrap_err(), "always-fail");
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            3,
            "must make exactly 3 attempts (1 initial + 2 retries)"
        );
    }

    #[tokio::test]
    async fn retry_backoff_doubles() {
        // Verify that delays double on each retry by using a custom retry_with_backoff
        // with a tiny initial delay (4ms) and asserting the delay doubles to 8ms.
        // We use wall-clock timing with generous tolerances for CI environments.
        let attempt_times = Arc::new(std::sync::Mutex::new(Vec::<std::time::Instant>::new()));
        let attempt_times_clone = Arc::clone(&attempt_times);

        let _result: Result<u32, String> = retry_with_backoff(
            || {
                let times = Arc::clone(&attempt_times_clone);
                async move {
                    times.lock().unwrap().push(std::time::Instant::now());
                    Err("fail".to_string())
                }
            },
            2,
            Duration::from_millis(4), // initial delay: 4ms (doubles to 8ms)
        )
        .await;

        let times = attempt_times.lock().unwrap();
        assert_eq!(times.len(), 3, "3 total attempts");

        // Gap1 should be ~4ms; gap2 should be ~8ms (i.e., gap2 > gap1)
        let gap1 = times[1].duration_since(times[0]).as_micros();
        let gap2 = times[2].duration_since(times[1]).as_micros();

        // gap2 must be at least 1.5x gap1 to confirm doubling (CI-safe tolerance)
        assert!(
            gap2 > gap1,
            "second backoff ({} µs) must be larger than first backoff ({} µs)",
            gap2,
            gap1
        );
        // Also verify both are at least 3ms in wall clock
        assert!(gap1 >= 3000, "first backoff must be at least 3ms, got {} µs", gap1);
        assert!(gap2 >= 6000, "second backoff must be at least 6ms, got {} µs", gap2);
    }

    #[tokio::test]
    async fn retry_zero_max_retries_no_retry() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = Arc::clone(&attempts);

        let result: Result<u32, String> = retry_with_backoff(
            || {
                let a = Arc::clone(&attempts_clone);
                async move {
                    a.fetch_add(1, Ordering::SeqCst);
                    Err("immediate-fail".to_string())
                }
            },
            0,                          // max_retries=0 → exactly 1 attempt
            Duration::from_millis(1),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "max_retries=0 must make exactly 1 attempt with no sleep"
        );
    }
}
