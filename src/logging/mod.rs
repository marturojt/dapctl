use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use ulid::Ulid;

pub const JSONL_SCHEMA_VERSION: u32 = 1;

pub struct InitOpts {
    pub run_id: Ulid,
    /// If set, human-readable log is also written here (no ANSI codes).
    pub human_log_file: Option<PathBuf>,
    /// Directory where `<run_id>.jsonl` will be written.
    pub jsonl_dir: PathBuf,
    pub verbosity: tracing::Level,
}

/// Returns (and creates) `$XDG_STATE_HOME/dapctl/runs/` (or platform equivalent).
pub fn default_jsonl_dir() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")
        .ok_or_else(|| anyhow::anyhow!("cannot determine state directory"))?;
    let path = dirs.data_local_dir().join("runs");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Initialise the global tracing subscriber. Must be called exactly once.
pub fn init(opts: InitOpts) -> anyhow::Result<()> {
    // --- JSONL sink (always on) ---
    let jsonl_path = opts.jsonl_dir.join(format!("{}.jsonl", opts.run_id));
    let jsonl_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&jsonl_path)
        .map_err(|e| anyhow::anyhow!("cannot open JSONL log {jsonl_path:?}: {e}"))?;
    let jsonl_layer = JsonlLayer::new(jsonl_file, opts.run_id.to_string());

    // --- Human sink: stderr ---
    let level_filter = tracing_subscriber::filter::LevelFilter::from_level(opts.verbosity);
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact()
        .with_filter(level_filter);

    // --- Human sink: optional file (no ANSI) ---
    let file_layer = if let Some(ref path) = opts.human_log_file {
        let file = File::create(path)
            .map_err(|e| anyhow::anyhow!("cannot open log file {path:?}: {e}"))?;
        Some(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .compact()
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .with_filter(level_filter),
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(jsonl_layer)
        .with(file_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("tracing already initialised: {e}"))?;

    tracing::info!(
        event = "start",
        run_id = %opts.run_id,
        schema_version = JSONL_SCHEMA_VERSION,
    );

    Ok(())
}

/// Write a `finish` event. Call once at the end of every command.
pub fn finish(ok: bool) {
    tracing::info!(event = "finish", ok);
}

// ---------------------------------------------------------------------------
// JSONL layer
// ---------------------------------------------------------------------------

struct JsonlLayer {
    writer: Mutex<BufWriter<File>>,
    run_id: String,
}

impl JsonlLayer {
    fn new(file: File, run_id: String) -> Self {
        Self {
            writer: Mutex::new(BufWriter::new(file)),
            run_id,
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for JsonlLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let ts = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());

        let level = event.metadata().level().as_str().to_lowercase();

        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let record = serde_json::json!({
            "schema_version": JSONL_SCHEMA_VERSION,
            "ts": ts,
            "level": level,
            "run_id": self.run_id,
            "fields": visitor.fields,
        });

        if let Ok(mut w) = self.writer.lock() {
            let _ = serde_json::to_writer(&mut *w, &record);
            let _ = w.write_all(b"\n");
            let _ = w.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// Field visitor
// ---------------------------------------------------------------------------

#[derive(Default)]
struct JsonVisitor {
    fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonVisitor {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        let v = serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null);
        self.fields.insert(field.name().to_owned(), v);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.fields
            .insert(field.name().to_owned(), value.to_string().into());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}").into());
    }
}
