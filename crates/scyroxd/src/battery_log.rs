use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use scyrox::{BatterySample, ConnectionMode, MouseError};
use serde::Serialize;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, SeekFrom};
use tokio::sync::Mutex;
use tracing::error;

const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BatteryRefreshSource {
    Periodic,
    DeviceConnected,
    ModeChanged,
    BatteryChanged,
    Rpc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BatteryLifecycleSource {
    Startup,
    Hotplug,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BatteryLogOpenError {
    #[error("battery_log_path must not alias daemon.toml or daemon.toml.tmp")]
    ReservedConfigPath,
    #[error("battery log I/O at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

struct BatteryLogSink {
    path: PathBuf,
    file: File,
    needs_record_boundary: bool,
}

pub(crate) struct PreparedBatteryLog(BatteryLogSink);

#[derive(Serialize)]
#[serde(untagged)]
enum BatteryLogRecord {
    Sample(SampleRecord),
    RefreshError(RefreshErrorRecord),
    DeviceConnected(DeviceConnectedRecord),
    DeviceDisconnected(DeviceDisconnectedRecord),
    ConnectionModeChanged(ConnectionModeChangedRecord),
}

pub(crate) struct PreparedBatteryLogRecord(BatteryLogRecord);

#[derive(Serialize)]
struct CommonRecord {
    schema_version: u8,
    event: &'static str,
    timestamp_unix_ms: i64,
    session_started_unix_ms: i64,
    session_elapsed_ms: u64,
}

#[derive(Serialize)]
struct SampleRecord {
    #[serde(flatten)]
    common: CommonRecord,
    source: BatteryRefreshSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt: Option<u8>,
    voltage_mv: u16,
    device_percentage: u8,
    estimated_percentage: u8,
    charging: bool,
    raw_response_hex: String,
}

#[derive(Serialize)]
struct RefreshErrorRecord {
    #[serde(flatten)]
    common: CommonRecord,
    source: BatteryRefreshSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt: Option<u8>,
    error_kind: &'static str,
    error_message: String,
}

#[derive(Serialize)]
struct DeviceConnectedRecord {
    #[serde(flatten)]
    common: CommonRecord,
    source: BatteryLifecycleSource,
    connection_mode: &'static str,
}

#[derive(Serialize)]
struct DeviceDisconnectedRecord {
    #[serde(flatten)]
    common: CommonRecord,
    source: BatteryLifecycleSource,
}

#[derive(Serialize)]
struct ConnectionModeChangedRecord {
    #[serde(flatten)]
    common: CommonRecord,
    source: BatteryLifecycleSource,
    from: &'static str,
    to: &'static str,
}

#[derive(Clone)]
pub(crate) struct BatteryLogger {
    session_started_at: Instant,
    session_started_unix_ms: i64,
    enabled: Arc<AtomicBool>,
    sink: Arc<Mutex<Option<BatteryLogSink>>>,
}

impl BatteryLogger {
    pub(crate) async fn new(
        path: Option<&Path>,
        reserved_paths: &[&Path],
    ) -> Result<Self, BatteryLogOpenError> {
        let logger = Self {
            session_started_at: Instant::now(),
            session_started_unix_ms: system_time_to_unix_ms(SystemTime::now()),
            enabled: Arc::new(AtomicBool::new(false)),
            sink: Arc::new(Mutex::new(None)),
        };
        let prepared = logger.prepare(path, reserved_paths).await?;
        logger.replace(prepared).await;
        Ok(logger)
    }

    pub(crate) async fn prepare(
        &self,
        path: Option<&Path>,
        reserved_paths: &[&Path],
    ) -> Result<Option<PreparedBatteryLog>, BatteryLogOpenError> {
        let Some(path) = path else {
            return Ok(None);
        };
        let path = path.to_path_buf();

        if reserved_paths
            .iter()
            .any(|reserved| lexically_normalize(path.as_path()) == lexically_normalize(reserved))
        {
            return Err(BatteryLogOpenError::ReservedConfigPath);
        }

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| BatteryLogOpenError::Io {
                    path: path.clone(),
                    source,
                })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|source| BatteryLogOpenError::Io {
                path: path.clone(),
                source,
            })?;

        reject_reserved_file_aliases(&path, reserved_paths).await?;

        let file_len = file
            .metadata()
            .await
            .map_err(|source| BatteryLogOpenError::Io {
                path: path.clone(),
                source,
            })?
            .len();
        let needs_record_boundary = if file_len == 0 {
            false
        } else {
            file.seek(SeekFrom::End(-1))
                .await
                .map_err(|source| BatteryLogOpenError::Io {
                    path: path.clone(),
                    source,
                })?;
            let mut final_byte = [0];
            file.read_exact(&mut final_byte)
                .await
                .map_err(|source| BatteryLogOpenError::Io {
                    path: path.clone(),
                    source,
                })?;
            final_byte[0] != b'\n'
        };

        Ok(Some(PreparedBatteryLog(BatteryLogSink {
            path,
            file,
            needs_record_boundary,
        })))
    }

    pub(crate) async fn replace(&self, prepared: Option<PreparedBatteryLog>) {
        let enabled = prepared.is_some();
        let mut sink = self.sink.lock().await;
        if let Some(old_sink) = sink.as_mut()
            && let Err(error) = old_sink.file.flush().await
        {
            error!(
                path = %old_sink.path.display(),
                %error,
                "failed to flush replaced battery log sink"
            );
        }
        *sink = prepared.map(|prepared| prepared.0);
        self.enabled.store(enabled, Ordering::Release);
    }

    pub(crate) fn prepare_sample_record(
        &self,
        source: BatteryRefreshSource,
        attempt: Option<u8>,
        sample: &BatterySample,
    ) -> Option<PreparedBatteryLogRecord> {
        let common = self.prepare_common_record("sample")?;
        Some(PreparedBatteryLogRecord(BatteryLogRecord::Sample(
            SampleRecord {
                common,
                source,
                attempt,
                voltage_mv: sample.status.voltage_mv,
                device_percentage: sample.device_percentage,
                estimated_percentage: sample.status.percentage,
                charging: sample.status.charging,
                raw_response_hex: hex::encode(&sample.raw_response),
            },
        )))
    }

    pub(crate) fn prepare_refresh_error_record(
        &self,
        source: BatteryRefreshSource,
        attempt: Option<u8>,
        error: &MouseError,
    ) -> Option<PreparedBatteryLogRecord> {
        let common = self.prepare_common_record("refresh_error")?;
        Some(PreparedBatteryLogRecord(BatteryLogRecord::RefreshError(
            RefreshErrorRecord {
                common,
                source,
                attempt,
                error_kind: mouse_error_kind(error),
                error_message: error.to_string(),
            },
        )))
    }

    pub(crate) async fn write_record(&self, record: PreparedBatteryLogRecord) {
        let mut bytes = match serde_json::to_vec(&record.0) {
            Ok(bytes) => bytes,
            Err(error) => {
                error!(%error, "failed to serialize battery log record");
                return;
            }
        };
        bytes.push(b'\n');

        let mut sink = self.sink.lock().await;
        let Some(sink) = sink.as_mut() else {
            return;
        };
        if let Err(error) =
            write_record_bytes(&mut sink.file, &mut sink.needs_record_boundary, &bytes).await
        {
            error!(
                path = %sink.path.display(),
                %error,
                "failed to write battery log record"
            );
        }
    }

    pub(crate) async fn log_device_connected(
        &self,
        source: BatteryLifecycleSource,
        mode: ConnectionMode,
    ) {
        let Some(common) = self.prepare_common_record("device_connected") else {
            return;
        };
        self.write_record(PreparedBatteryLogRecord(BatteryLogRecord::DeviceConnected(
            DeviceConnectedRecord {
                common,
                source,
                connection_mode: connection_mode_name(mode),
            },
        )))
        .await;
    }

    pub(crate) async fn log_device_disconnected(&self, source: BatteryLifecycleSource) {
        let Some(common) = self.prepare_common_record("device_disconnected") else {
            return;
        };
        self.write_record(PreparedBatteryLogRecord(
            BatteryLogRecord::DeviceDisconnected(DeviceDisconnectedRecord { common, source }),
        ))
        .await;
    }

    pub(crate) async fn log_connection_mode_changed(
        &self,
        source: BatteryLifecycleSource,
        from: ConnectionMode,
        to: ConnectionMode,
    ) {
        let Some(common) = self.prepare_common_record("connection_mode_changed") else {
            return;
        };
        self.write_record(PreparedBatteryLogRecord(
            BatteryLogRecord::ConnectionModeChanged(ConnectionModeChangedRecord {
                common,
                source,
                from: connection_mode_name(from),
                to: connection_mode_name(to),
            }),
        ))
        .await;
    }

    fn prepare_common_record(&self, event: &'static str) -> Option<CommonRecord> {
        if !self.enabled.load(Ordering::Acquire) {
            return None;
        }
        Some(CommonRecord {
            schema_version: SCHEMA_VERSION,
            event,
            timestamp_unix_ms: system_time_to_unix_ms(SystemTime::now()),
            session_started_unix_ms: self.session_started_unix_ms,
            session_elapsed_ms: duration_to_millis(self.session_started_at.elapsed()),
        })
    }
}

async fn reject_reserved_file_aliases(
    path: &Path,
    reserved_paths: &[&Path],
) -> Result<(), BatteryLogOpenError> {
    let path = path.to_path_buf();
    let reserved_paths = reserved_paths
        .iter()
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>();
    let error_path = path.clone();

    tokio::task::spawn_blocking(move || {
        for reserved_path in reserved_paths {
            match same_file::is_same_file(&path, &reserved_path) {
                Ok(true) => return Err(BatteryLogOpenError::ReservedConfigPath),
                Ok(false) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    match std::fs::metadata(&reserved_path) {
                        Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                        Err(source) => {
                            return Err(BatteryLogOpenError::Io {
                                path: path.clone(),
                                source,
                            });
                        }
                        Ok(_) => {
                            return Err(BatteryLogOpenError::Io {
                                path: path.clone(),
                                source,
                            });
                        }
                    }
                }
                Err(source) => {
                    return Err(BatteryLogOpenError::Io {
                        path: path.clone(),
                        source,
                    });
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|source| BatteryLogOpenError::Io {
        path: error_path,
        source: io::Error::other(source),
    })?
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn system_time_to_unix_ms(time: SystemTime) -> i64 {
    let milliseconds = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i128,
        Err(error) => -(error.duration().as_millis() as i128),
    };
    milliseconds.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn connection_mode_name(mode: ConnectionMode) -> &'static str {
    match mode {
        ConnectionMode::Wired => "wired",
        ConnectionMode::Wireless => "wireless",
    }
}

fn mouse_error_kind(error: &MouseError) -> &'static str {
    match error {
        MouseError::NotFound { .. } => "not_found",
        MouseError::Hid(_) => "hid",
        MouseError::InvalidPollingRate(_) => "invalid_polling_rate",
        MouseError::InvalidLiftOffDistance(_) => "invalid_lift_off_distance",
        MouseError::InvalidSleepTimeout(_) => "invalid_sleep_timeout",
        MouseError::InvalidDpiStage(_) => "invalid_dpi_stage",
        MouseError::InvalidDpiValue(_) => "invalid_dpi_value",
        MouseError::InvalidDebounceTime(_) => "invalid_debounce_time",
        MouseError::InvalidProfile(_) => "invalid_profile",
        MouseError::Timeout => "timeout",
        MouseError::UnexpectedResponse { .. } => "unexpected_response",
        MouseError::InsufficientData { .. } => "insufficient_data",
        MouseError::NotSupported => "not_supported",
        MouseError::DeviceOffline => "device_offline",
        MouseError::Disconnected => "disconnected",
        MouseError::ChannelClosed => "channel_closed",
        MouseError::TaskPanic => "task_panic",
    }
}

async fn write_record_bytes<W: AsyncWrite + Unpin>(
    writer: &mut W,
    needs_record_boundary: &mut bool,
    record: &[u8],
) -> io::Result<()> {
    if *needs_record_boundary {
        writer.write_all(b"\n").await?;
    }
    if let Err(error) = writer.write_all(record).await {
        *needs_record_boundary = true;
        return Err(error);
    }
    if let Err(error) = writer.flush().await {
        *needs_record_boundary = true;
        return Err(error);
    }
    *needs_record_boundary = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::task::{Context, Poll};

    use serde_json::{Value, json};
    use tokio::io::AsyncWrite;

    use super::*;

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scyroxd-battery-log-tests-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sample() -> BatterySample {
        BatterySample {
            status: scyrox::BatteryStatus {
                voltage_mv: 3700,
                percentage: 42,
                charging: true,
            },
            device_percentage: 77,
            raw_response: vec![0x08, 0x04, 0x00, 0xab, 0xcd],
        }
    }

    async fn write_sample(logger: &BatteryLogger, source: BatteryRefreshSource) {
        let record = logger
            .prepare_sample_record(source, None, &sample())
            .unwrap();
        logger.write_record(record).await;
    }

    #[tokio::test]
    async fn disabled_logging_creates_no_file() {
        let test_dir = TestDir::new();
        let path = test_dir.path("battery.jsonl");
        let logger = BatteryLogger::new(None, &[]).await.unwrap();

        assert!(
            logger
                .prepare_sample_record(BatteryRefreshSource::Rpc, None, &sample())
                .is_none()
        );
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn sample_is_one_flat_newline_terminated_json_object() {
        let test_dir = TestDir::new();
        let path = test_dir.path("battery.jsonl");
        let logger = BatteryLogger::new(Some(&path), &[]).await.unwrap();

        write_sample(&logger, BatteryRefreshSource::Rpc).await;

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.ends_with('\n'));
        assert_eq!(contents.lines().count(), 1);
        let value: Value = serde_json::from_str(contents.trim_end()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["event"], "sample");
        assert!(value["timestamp_unix_ms"].is_i64());
        assert!(value["session_started_unix_ms"].is_i64());
        assert!(value["session_elapsed_ms"].is_u64());
        assert_eq!(value["source"], "rpc");
        assert_eq!(value["voltage_mv"], 3700);
        assert_eq!(value["device_percentage"], 77);
        assert_eq!(value["estimated_percentage"], 42);
        assert_eq!(value["charging"], true);
        assert_eq!(value["raw_response_hex"], "080400abcd");
        assert!(value.get("attempt").is_none());
        assert!(value.get("data").is_none());
    }

    #[tokio::test]
    async fn reopening_appends_without_truncating() {
        let test_dir = TestDir::new();
        let path = test_dir.path("battery.jsonl");
        let first_logger = BatteryLogger::new(Some(&path), &[]).await.unwrap();
        write_sample(&first_logger, BatteryRefreshSource::Periodic).await;
        drop(first_logger);

        let second_logger = BatteryLogger::new(Some(&path), &[]).await.unwrap();
        write_sample(&second_logger, BatteryRefreshSource::Rpc).await;

        let sources = fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["source"].clone())
            .collect::<Vec<_>>();
        assert_eq!(sources, [json!("periodic"), json!("rpc")]);
    }

    #[tokio::test]
    async fn partial_final_line_is_separated_from_next_record() {
        let test_dir = TestDir::new();
        let path = test_dir.path("battery.jsonl");
        fs::write(&path, b"{\"broken\":").unwrap();
        let logger = BatteryLogger::new(Some(&path), &[]).await.unwrap();

        logger
            .log_device_disconnected(BatteryLifecycleSource::Hotplug)
            .await;

        let contents = fs::read_to_string(path).unwrap();
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "{\"broken\":");
        assert_eq!(
            serde_json::from_str::<Value>(lines[1]).unwrap()["event"],
            "device_disconnected"
        );
    }

    #[tokio::test]
    async fn directory_path_fails_to_open() {
        let test_dir = TestDir::new();
        let error = BatteryLogger::new(Some(&test_dir.path), &[])
            .await
            .err()
            .unwrap();

        assert!(matches!(error, BatteryLogOpenError::Io { .. }));
    }

    #[tokio::test]
    async fn direct_and_normalized_reserved_paths_are_rejected() {
        let test_dir = TestDir::new();
        let config_path = test_dir.path("daemon.toml");
        let temp_path = test_dir.path("daemon.toml.tmp");
        let normalized_path = test_dir.path("nested/../daemon.toml");

        for path in [&config_path, &temp_path, &normalized_path] {
            let error = BatteryLogger::new(Some(path), &[&config_path, &temp_path])
                .await
                .err()
                .unwrap();
            assert!(matches!(error, BatteryLogOpenError::ReservedConfigPath));
        }
    }

    #[tokio::test]
    async fn hard_link_to_reserved_path_is_rejected() {
        let test_dir = TestDir::new();
        let config_path = test_dir.path("daemon.toml");
        let alias_path = test_dir.path("battery.jsonl");
        fs::write(&config_path, b"config").unwrap();
        fs::hard_link(&config_path, &alias_path).unwrap();

        let error = BatteryLogger::new(Some(&alias_path), &[&config_path])
            .await
            .err()
            .unwrap();

        assert!(matches!(error, BatteryLogOpenError::ReservedConfigPath));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_to_reserved_path_is_rejected() {
        let test_dir = TestDir::new();
        let config_path = test_dir.path("daemon.toml");
        let alias_path = test_dir.path("battery.jsonl");
        fs::write(&config_path, b"config").unwrap();
        std::os::unix::fs::symlink(&config_path, &alias_path).unwrap();

        let error = BatteryLogger::new(Some(&alias_path), &[&config_path])
            .await
            .err()
            .unwrap();

        assert!(matches!(error, BatteryLogOpenError::ReservedConfigPath));
    }

    #[tokio::test]
    async fn source_mode_and_error_kind_literals_match_schema() {
        assert_eq!(
            serde_json::to_value([
                BatteryRefreshSource::Periodic,
                BatteryRefreshSource::DeviceConnected,
                BatteryRefreshSource::ModeChanged,
                BatteryRefreshSource::BatteryChanged,
                BatteryRefreshSource::Rpc,
            ])
            .unwrap(),
            json!([
                "periodic",
                "device_connected",
                "mode_changed",
                "battery_changed",
                "rpc"
            ])
        );
        assert_eq!(
            serde_json::to_value([
                BatteryLifecycleSource::Startup,
                BatteryLifecycleSource::Hotplug,
            ])
            .unwrap(),
            json!(["startup", "hotplug"])
        );
        assert_eq!(
            [
                connection_mode_name(ConnectionMode::Wired),
                connection_mode_name(ConnectionMode::Wireless),
            ],
            ["wired", "wireless"]
        );

        let errors = [
            MouseError::NotFound {
                vid: 0,
                pids: Vec::new(),
            },
            MouseError::Hid(hidapi::HidError::HidApiError {
                message: "test".to_owned(),
            }),
            MouseError::InvalidPollingRate(0),
            MouseError::InvalidLiftOffDistance(0),
            MouseError::InvalidSleepTimeout(0),
            MouseError::InvalidDpiStage(0),
            MouseError::InvalidDpiValue(0),
            MouseError::InvalidDebounceTime(0),
            MouseError::InvalidProfile(0),
            MouseError::Timeout,
            MouseError::UnexpectedResponse {
                expected: 0,
                got: 1,
            },
            MouseError::InsufficientData { need: 1, got: 0 },
            MouseError::NotSupported,
            MouseError::DeviceOffline,
            MouseError::Disconnected,
            MouseError::ChannelClosed,
            MouseError::TaskPanic,
        ];
        assert_eq!(
            errors.map(|error| mouse_error_kind(&error)),
            [
                "not_found",
                "hid",
                "invalid_polling_rate",
                "invalid_lift_off_distance",
                "invalid_sleep_timeout",
                "invalid_dpi_stage",
                "invalid_dpi_value",
                "invalid_debounce_time",
                "invalid_profile",
                "timeout",
                "unexpected_response",
                "insufficient_data",
                "not_supported",
                "device_offline",
                "disconnected",
                "channel_closed",
                "task_panic",
            ]
        );
    }

    struct FailOnceWriter {
        bytes: Vec<u8>,
        fail_write: bool,
        fail_flush: bool,
    }

    impl AsyncWrite for FailOnceWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.fail_write {
                self.fail_write = false;
                return Poll::Ready(Err(io::Error::other("injected write failure")));
            }
            self.bytes.extend_from_slice(bytes);
            Poll::Ready(Ok(bytes.len()))
        }

        fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            if self.fail_flush {
                self.fail_flush = false;
                return Poll::Ready(Err(io::Error::other("injected flush failure")));
            }
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn failed_write_is_not_replayed_and_next_record_gets_boundary() {
        let mut writer = FailOnceWriter {
            bytes: Vec::new(),
            fail_write: true,
            fail_flush: false,
        };
        let mut needs_boundary = false;

        assert!(
            write_record_bytes(&mut writer, &mut needs_boundary, b"first\n")
                .await
                .is_err()
        );
        write_record_bytes(&mut writer, &mut needs_boundary, b"second\n")
            .await
            .unwrap();

        assert_eq!(writer.bytes, b"\nsecond\n");
    }

    #[tokio::test]
    async fn failed_flush_is_not_replayed_and_next_record_gets_boundary() {
        let mut writer = FailOnceWriter {
            bytes: Vec::new(),
            fail_write: false,
            fail_flush: true,
        };
        let mut needs_boundary = false;

        assert!(
            write_record_bytes(&mut writer, &mut needs_boundary, b"first\n")
                .await
                .is_err()
        );
        write_record_bytes(&mut writer, &mut needs_boundary, b"second\n")
            .await
            .unwrap();

        assert_eq!(writer.bytes, b"first\n\nsecond\n");
    }
}
