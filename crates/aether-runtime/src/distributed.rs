use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tracing::warn;
use uuid::Uuid;

use crate::concurrency::{ConcurrencyGate, ConcurrencyPermit};
use crate::metrics::{MetricKind, MetricLabel, MetricSample};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DistributedConcurrencyError {
    #[error("distributed concurrency gate {gate} is saturated at {limit}")]
    Saturated { gate: &'static str, limit: usize },
    #[error("distributed concurrency gate {gate} is unavailable: {message}")]
    Unavailable {
        gate: &'static str,
        limit: usize,
        message: String,
    },
    #[error("{0}")]
    InvalidConfiguration(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistributedConcurrencySnapshot {
    pub limit: usize,
    pub in_flight: usize,
    pub available_permits: usize,
    pub high_watermark: usize,
    pub rejected: u64,
}

impl DistributedConcurrencySnapshot {
    pub fn to_metric_samples(&self, gate: &'static str) -> Vec<MetricSample> {
        let labels = vec![MetricLabel::new("gate", gate)];
        vec![
            MetricSample::new(
                "concurrency_in_flight",
                "Current number of in-flight operations guarded by the concurrency gate.",
                MetricKind::Gauge,
                self.in_flight as u64,
            )
            .with_labels(labels.clone()),
            MetricSample::new(
                "concurrency_available_permits",
                "Currently available permits for the concurrency gate.",
                MetricKind::Gauge,
                self.available_permits as u64,
            )
            .with_labels(labels.clone()),
            MetricSample::new(
                "concurrency_high_watermark",
                "Highest observed in-flight count for the concurrency gate.",
                MetricKind::Gauge,
                self.high_watermark as u64,
            )
            .with_labels(labels.clone()),
            MetricSample::new(
                "concurrency_rejected_total",
                "Number of operations rejected by the concurrency gate.",
                MetricKind::Counter,
                self.rejected,
            )
            .with_labels(labels),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisDistributedConcurrencyConfig {
    pub url: String,
    pub key_prefix: Option<String>,
    pub lease_ttl_ms: u64,
    pub renew_interval_ms: u64,
    pub command_timeout_ms: Option<u64>,
}

impl Default for RedisDistributedConcurrencyConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            key_prefix: None,
            lease_ttl_ms: 30_000,
            renew_interval_ms: 10_000,
            command_timeout_ms: Some(1_000),
        }
    }
}

impl RedisDistributedConcurrencyConfig {
    fn validate(&self) -> Result<(), DistributedConcurrencyError> {
        let raw = self.url.trim();
        if raw.is_empty() {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency redis url cannot be empty".to_string(),
            ));
        }
        url::Url::parse(raw).map_err(|err| {
            DistributedConcurrencyError::InvalidConfiguration(format!(
                "invalid distributed concurrency redis url: {err}"
            ))
        })?;
        if self.lease_ttl_ms == 0 {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency lease_ttl_ms must be positive".to_string(),
            ));
        }
        if self.renew_interval_ms == 0 {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency renew_interval_ms must be positive".to_string(),
            ));
        }
        if self.renew_interval_ms >= self.lease_ttl_ms {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency renew_interval_ms must be smaller than lease_ttl_ms"
                    .to_string(),
            ));
        }
        if matches!(self.command_timeout_ms, Some(0)) {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency command_timeout_ms must be positive".to_string(),
            ));
        }
        Ok(())
    }

    fn semaphore_key(&self, gate: &'static str) -> String {
        prefixed_key(self.key_prefix.as_deref(), &format!("admission:{gate}"))
    }
}

#[derive(Debug)]
enum DistributedConcurrencyBackend {
    InMemory(Arc<ConcurrencyGate>),
    Redis(Arc<RedisDistributedState>),
}

#[derive(Debug)]
struct DistributedConcurrencyState {
    gate: &'static str,
    limit: usize,
    backend: DistributedConcurrencyBackend,
}

#[derive(Debug, Clone)]
pub struct DistributedConcurrencyGate {
    state: Arc<DistributedConcurrencyState>,
}

impl DistributedConcurrencyGate {
    pub fn new_in_memory(gate: &'static str, limit: usize) -> Self {
        assert!(
            limit > 0,
            "distributed concurrency gate limit must be positive"
        );
        Self {
            state: Arc::new(DistributedConcurrencyState {
                gate,
                limit,
                backend: DistributedConcurrencyBackend::InMemory(Arc::new(ConcurrencyGate::new(
                    gate, limit,
                ))),
            }),
        }
    }

    pub fn new_redis(
        gate: &'static str,
        limit: usize,
        config: RedisDistributedConcurrencyConfig,
    ) -> Result<Self, DistributedConcurrencyError> {
        if limit == 0 {
            return Err(DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency gate limit must be positive".to_string(),
            ));
        }
        config.validate()?;
        let client = redis::Client::open(config.url.clone()).map_err(|err| {
            DistributedConcurrencyError::InvalidConfiguration(format!(
                "failed to build distributed concurrency redis client: {err}"
            ))
        })?;

        Ok(Self {
            state: Arc::new(DistributedConcurrencyState {
                gate,
                limit,
                backend: DistributedConcurrencyBackend::Redis(Arc::new(RedisDistributedState {
                    gate,
                    limit,
                    client,
                    key: config.semaphore_key(gate),
                    lease_ttl_ms: config.lease_ttl_ms,
                    renew_interval_ms: config.renew_interval_ms,
                    command_timeout_ms: config.command_timeout_ms,
                    high_watermark: AtomicUsize::new(0),
                    rejected: AtomicU64::new(0),
                })),
            }),
        })
    }

    pub fn gate(&self) -> &'static str {
        self.state.gate
    }

    pub fn limit(&self) -> usize {
        self.state.limit
    }

    pub async fn try_acquire(
        &self,
    ) -> Result<DistributedConcurrencyPermit, DistributedConcurrencyError> {
        match &self.state.backend {
            DistributedConcurrencyBackend::InMemory(gate) => gate
                .try_acquire()
                .map(DistributedConcurrencyPermit::from_in_memory)
                .map_err(|err| match err {
                    crate::ConcurrencyError::Saturated { gate, limit } => {
                        DistributedConcurrencyError::Saturated { gate, limit }
                    }
                    crate::ConcurrencyError::Closed { gate } => {
                        DistributedConcurrencyError::Unavailable {
                            gate,
                            limit: self.state.limit,
                            message: "in-memory distributed concurrency gate is closed".to_string(),
                        }
                    }
                }),
            DistributedConcurrencyBackend::Redis(state) => state.try_acquire().await,
        }
    }

    pub async fn snapshot(
        &self,
    ) -> Result<DistributedConcurrencySnapshot, DistributedConcurrencyError> {
        match &self.state.backend {
            DistributedConcurrencyBackend::InMemory(gate) => {
                let snapshot = gate.snapshot();
                Ok(DistributedConcurrencySnapshot {
                    limit: snapshot.limit,
                    in_flight: snapshot.in_flight,
                    available_permits: snapshot.available_permits,
                    high_watermark: snapshot.high_watermark,
                    rejected: snapshot.rejected,
                })
            }
            DistributedConcurrencyBackend::Redis(state) => state.snapshot().await,
        }
    }
}

#[derive(Debug)]
pub struct DistributedConcurrencyPermit {
    inner: DistributedConcurrencyPermitInner,
}

#[derive(Debug)]
enum DistributedConcurrencyPermitInner {
    InMemory(ConcurrencyPermit),
    Redis {
        state: Arc<RedisDistributedState>,
        token: String,
        renew_task: JoinHandle<()>,
    },
}

impl DistributedConcurrencyPermit {
    fn from_in_memory(permit: ConcurrencyPermit) -> Self {
        Self {
            inner: DistributedConcurrencyPermitInner::InMemory(permit),
        }
    }

    fn from_redis(
        state: Arc<RedisDistributedState>,
        token: String,
        renew_task: JoinHandle<()>,
    ) -> Self {
        Self {
            inner: DistributedConcurrencyPermitInner::Redis {
                state,
                token,
                renew_task,
            },
        }
    }
}

impl Drop for DistributedConcurrencyPermit {
    fn drop(&mut self) {
        match &mut self.inner {
            DistributedConcurrencyPermitInner::InMemory(_permit) => {}
            DistributedConcurrencyPermitInner::Redis {
                state,
                token,
                renew_task,
            } => {
                renew_task.abort();
                let state = Arc::clone(state);
                let token = token.clone();
                tokio::spawn(async move {
                    if let Err(err) = state.release(&token).await {
                        warn!(
                            gate = state.gate,
                            error = %err,
                            "failed to release distributed concurrency permit"
                        );
                    }
                });
            }
        }
    }
}

#[derive(Debug)]
struct RedisDistributedState {
    gate: &'static str,
    limit: usize,
    client: redis::Client,
    key: String,
    lease_ttl_ms: u64,
    renew_interval_ms: u64,
    command_timeout_ms: Option<u64>,
    high_watermark: AtomicUsize,
    rejected: AtomicU64,
}

impl RedisDistributedState {
    async fn try_acquire(
        self: &Arc<Self>,
    ) -> Result<DistributedConcurrencyPermit, DistributedConcurrencyError> {
        let token = format!("{}:{}", self.gate, Uuid::new_v4());
        let now_ms = unix_time_ms();
        let expires_at_ms = now_ms.saturating_add(self.lease_ttl_ms);
        let key = self.key.clone();
        let result: (i64, i64) = self
            .run_with_timeout("acquire", async {
                let mut connection = self
                    .client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|err| self.unavailable(format!("connect failed: {err}")))?;
                redis::Script::new(
                    "redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1]); \
                     local count = redis.call('ZCARD', KEYS[1]); \
                     if count >= tonumber(ARGV[3]) then \
                        redis.call('PEXPIRE', KEYS[1], ARGV[5]); \
                        return {0, count}; \
                     end; \
                     redis.call('ZADD', KEYS[1], ARGV[2], ARGV[4]); \
                     count = redis.call('ZCARD', KEYS[1]); \
                     redis.call('PEXPIRE', KEYS[1], ARGV[5]); \
                     return {1, count};",
                )
                .key(&key)
                .arg(now_ms as i64)
                .arg(expires_at_ms as i64)
                .arg(self.limit as i64)
                .arg(&token)
                .arg(self.lease_ttl_ms as i64)
                .invoke_async::<(i64, i64)>(&mut connection)
                .await
                .map_err(|err| self.unavailable(format!("acquire failed: {err}")))
            })
            .await?;

        let acquired = result.0 > 0;
        let in_flight = result.1.max(0) as usize;
        self.observe_in_flight(in_flight);

        if !acquired {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(DistributedConcurrencyError::Saturated {
                gate: self.gate,
                limit: self.limit,
            });
        }

        let renew_state = Arc::clone(self);
        let renew_token = token.clone();
        let renew_task = tokio::spawn(async move {
            let interval = Duration::from_millis(renew_state.renew_interval_ms);
            loop {
                tokio::time::sleep(interval).await;
                if let Err(err) = renew_state.renew(&renew_token).await {
                    warn!(
                        gate = renew_state.gate,
                        error = %err,
                        "failed to renew distributed concurrency permit"
                    );
                    break;
                }
            }
        });

        Ok(DistributedConcurrencyPermit::from_redis(
            Arc::clone(self),
            token,
            renew_task,
        ))
    }

    async fn snapshot(
        &self,
    ) -> Result<DistributedConcurrencySnapshot, DistributedConcurrencyError> {
        let in_flight = self.live_count().await?;
        Ok(DistributedConcurrencySnapshot {
            limit: self.limit,
            in_flight,
            available_permits: self.limit.saturating_sub(in_flight),
            high_watermark: self.high_watermark.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
        })
    }

    async fn renew(&self, token: &str) -> Result<(), DistributedConcurrencyError> {
        let now_ms = unix_time_ms();
        let expires_at_ms = now_ms.saturating_add(self.lease_ttl_ms);
        let key = self.key.clone();
        let renewed = self
            .run_with_timeout("renew", async {
                let mut connection = self
                    .client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|err| self.unavailable(format!("connect failed: {err}")))?;
                redis::Script::new(
                    "redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1]); \
                     local score = redis.call('ZSCORE', KEYS[1], ARGV[2]); \
                     if not score then \
                        return 0; \
                     end; \
                     redis.call('ZADD', KEYS[1], 'XX', ARGV[3], ARGV[2]); \
                     redis.call('PEXPIRE', KEYS[1], ARGV[4]); \
                     return 1;",
                )
                .key(&key)
                .arg(now_ms as i64)
                .arg(token)
                .arg(expires_at_ms as i64)
                .arg(self.lease_ttl_ms as i64)
                .invoke_async::<i64>(&mut connection)
                .await
                .map_err(|err| self.unavailable(format!("renew failed: {err}")))
            })
            .await?;

        if renewed == 0 {
            return Err(self.unavailable("lease token expired".to_string()));
        }
        Ok(())
    }

    async fn release(&self, token: &str) -> Result<(), DistributedConcurrencyError> {
        let key = self.key.clone();
        self.run_with_timeout("release", async {
            let mut connection = self
                .client
                .get_multiplexed_async_connection()
                .await
                .map_err(|err| self.unavailable(format!("connect failed: {err}")))?;
            redis::Script::new(
                "local removed = redis.call('ZREM', KEYS[1], ARGV[1]); \
                 if removed > 0 and redis.call('ZCARD', KEYS[1]) == 0 then \
                    redis.call('DEL', KEYS[1]); \
                 end; \
                 return removed;",
            )
            .key(&key)
            .arg(token)
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(|err| self.unavailable(format!("release failed: {err}")))?;
            Ok(())
        })
        .await
    }

    async fn live_count(&self) -> Result<usize, DistributedConcurrencyError> {
        let now_ms = unix_time_ms();
        let key = self.key.clone();
        let count = self
            .run_with_timeout("snapshot", async {
                let mut connection = self
                    .client
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(|err| self.unavailable(format!("connect failed: {err}")))?;
                redis::Script::new(
                    "redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1]); \
                     return redis.call('ZCARD', KEYS[1]);",
                )
                .key(&key)
                .arg(now_ms as i64)
                .invoke_async::<i64>(&mut connection)
                .await
                .map_err(|err| self.unavailable(format!("snapshot failed: {err}")))
            })
            .await?
            .max(0) as usize;
        self.observe_in_flight(count);
        Ok(count)
    }

    async fn run_with_timeout<T, F>(
        &self,
        operation: &'static str,
        future: F,
    ) -> Result<T, DistributedConcurrencyError>
    where
        F: std::future::Future<Output = Result<T, DistributedConcurrencyError>>,
    {
        if let Some(timeout_ms) = self.command_timeout_ms {
            tokio::time::timeout(Duration::from_millis(timeout_ms), future)
                .await
                .map_err(|_| {
                    self.unavailable(format!(
                        "{operation} exceeded {timeout_ms}ms command timeout"
                    ))
                })?
        } else {
            future.await
        }
    }

    fn unavailable(&self, message: String) -> DistributedConcurrencyError {
        DistributedConcurrencyError::Unavailable {
            gate: self.gate,
            limit: self.limit,
            message,
        }
    }

    fn observe_in_flight(&self, in_flight: usize) {
        let mut observed = self.high_watermark.load(Ordering::Acquire);
        while in_flight > observed {
            match self.high_watermark.compare_exchange_weak(
                observed,
                in_flight,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(next) => observed = next,
            }
        }
    }
}

fn prefixed_key(prefix: Option<&str>, raw_key: &str) -> String {
    let prefix = prefix.unwrap_or_default().trim().trim_matches(':');
    if prefix.is_empty() {
        raw_key.trim_matches(':').to_string()
    } else {
        format!("{prefix}:{}", raw_key.trim_matches(':'))
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::{
        DistributedConcurrencyError, DistributedConcurrencyGate, RedisDistributedConcurrencyConfig,
    };

    #[tokio::test]
    async fn shared_in_memory_gate_rejects_second_acquire() {
        let gate = DistributedConcurrencyGate::new_in_memory("shared", 1);
        let permit = gate.try_acquire().await.expect("first permit");

        let error = gate
            .try_acquire()
            .await
            .expect_err("second permit should fail");
        assert_eq!(
            error,
            DistributedConcurrencyError::Saturated {
                gate: "shared",
                limit: 1,
            }
        );

        let snapshot = gate.snapshot().await.expect("snapshot should build");
        assert_eq!(snapshot.in_flight, 1);
        assert_eq!(snapshot.available_permits, 0);
        assert_eq!(snapshot.high_watermark, 1);
        assert_eq!(snapshot.rejected, 1);

        drop(permit);
        let snapshot = gate.snapshot().await.expect("snapshot should build");
        assert_eq!(snapshot.in_flight, 0);
    }

    #[test]
    fn rejects_invalid_redis_config() {
        let error = DistributedConcurrencyGate::new_redis(
            "shared",
            1,
            RedisDistributedConcurrencyConfig {
                url: "redis://127.0.0.1/0".to_string(),
                key_prefix: Some("aether".to_string()),
                lease_ttl_ms: 10_000,
                renew_interval_ms: 10_000,
                command_timeout_ms: Some(1_000),
            },
        )
        .expect_err("equal renew interval should fail");
        assert_eq!(
            error,
            DistributedConcurrencyError::InvalidConfiguration(
                "distributed concurrency renew_interval_ms must be smaller than lease_ttl_ms"
                    .to_string()
            )
        );
    }

    #[test]
    fn builds_redis_gate_without_touching_network() {
        let gate = DistributedConcurrencyGate::new_redis(
            "gateway_requests_distributed",
            2,
            RedisDistributedConcurrencyConfig {
                url: "redis://127.0.0.1/0".to_string(),
                key_prefix: Some("aether".to_string()),
                lease_ttl_ms: 15_000,
                renew_interval_ms: 5_000,
                command_timeout_ms: Some(1_000),
            },
        )
        .expect("redis gate should build");

        assert_eq!(gate.gate(), "gateway_requests_distributed");
        assert_eq!(gate.limit(), 2);
    }
}
