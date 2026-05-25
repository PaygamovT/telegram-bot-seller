use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Sliding Window Rate Limiter to protect the bot against flooding
pub struct RateLimiter {
    requests: Mutex<HashMap<i64, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    /// Creates a new RateLimiter instance
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            max_requests,
            window,
        }
    }

    /// Checks if the request is within the allowed limits.
    /// Returns `true` if allowed, `false` if throttled.
    pub fn check(&self, user_id: i64) -> bool {
        let mut map = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let now = Instant::now();
        let user_reqs = map.entry(user_id).or_default();

        // Retain only requests inside the active sliding window duration
        user_reqs.retain(|&time| {
            now.checked_duration_since(time)
                .map(|dur| dur < self.window)
                .unwrap_or(true)
        });

        if user_reqs.len() < self.max_requests {
            user_reqs.push(now);
            true
        } else {
            false
        }
    }
}
