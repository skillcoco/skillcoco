use crate::ai::service::{AIServiceRequest, AIServiceResponse};
use crate::auth::AuthState;

/// AI request with exponential-backoff retry.
///
/// Wave 1 (03-02 Task 2) implements the real retry loop.
/// This stub returns Err immediately so Wave 0 tests FAIL.
pub async fn ai_request_with_retry(
    _auth: &AuthState,
    _request: AIServiceRequest,
    _max_retries: u8,
) -> Result<AIServiceResponse, String> {
    Err("not implemented — Wave 1 (03-02) lands the retry loop".to_string())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        // Wave 1 will rewrite this once the retry loop is real.
        // For now: stub returns Err, so the expectation of Ok is not met — test FAILS.
        panic!("WAVE 1 STUB — implement retry then assert success-on-second-attempt");
    }

    #[tokio::test]
    async fn retry_fails_after_max_retries() {
        // Wave 1 will rewrite this once the retry loop is real.
        panic!("WAVE 1 STUB — implement retry then assert fail-after-max-retries");
    }
}
