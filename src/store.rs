use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::model::{Artifact, Event, Finding, ReportArtifact, RunId, Sample};
use crate::util::time::unix_timestamp_seconds;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub session_id: RunId,
    pub session_name: String,
    pub created_at: u64,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct RunStore {
    root: PathBuf,
}

impl RunStore {
    pub fn create(config: &Config, command: Vec<String>) -> Result<Self> {
        let session_id = RunId(format!(
            "{}-{}",
            config.session.name,
            unix_timestamp_seconds()
        ));
        let root = config.session.output_dir.join(&session_id.0);
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create session directory {}", root.display()))?;
        let store = Self { root };
        let manifest = RunManifest {
            session_id,
            session_name: config.session.name.clone(),
            created_at: unix_timestamp_seconds(),
            command,
            exit_code: None,
        };
        store.write_json("manifest.json", &manifest)?;
        Ok(store)
    }

    pub fn open(base_dir: &Path, session_id: &str) -> Self {
        Self {
            root: base_dir.join(session_id),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn session_id(&self) -> Option<String> {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    pub fn artifact_paths(&self) -> RunArtifactPaths {
        RunArtifactPaths {
            manifest: self.root.join("manifest.json"),
            samples: self.root.join("samples.jsonl"),
            events: self.root.join("events.jsonl"),
            artifacts: self.root.join("artifacts.jsonl"),
            findings: self.root.join("findings.json"),
            json_report: self.root.join("report.json"),
            perfetto_trace: self.root.join("trace.json"),
        }
    }

    pub fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    pub fn write_artifact_bytes(&self, name: &str, bytes: &[u8]) -> Result<()> {
        fs::create_dir_all(self.artifacts_dir())?;
        fs::write(self.artifacts_dir().join(name), bytes)?;
        Ok(())
    }

    pub fn append_samples(&self, samples: &[Sample]) -> Result<()> {
        self.append_jsonl("samples.jsonl", samples)
    }

    pub fn append_events(&self, events: &[Event]) -> Result<()> {
        self.append_jsonl("events.jsonl", events)
    }

    pub fn append_artifacts(&self, artifacts: &[Artifact]) -> Result<()> {
        self.append_jsonl("artifacts.jsonl", artifacts)
    }

    pub fn write_findings(&self, findings: &[Finding]) -> Result<()> {
        self.write_json("findings.json", findings)
    }

    pub fn write_report_artifacts(&self, reports: &[ReportArtifact]) -> Result<()> {
        self.write_json("reports.json", reports)
    }

    pub fn read_samples(&self) -> Result<Vec<Sample>> {
        self.read_jsonl("samples.jsonl")
    }

    pub fn read_findings(&self) -> Result<Vec<Finding>> {
        let path = self.root.join("findings.json");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn update_exit_code(&self, exit_code: Option<i32>) -> Result<()> {
        let manifest_path = self.root.join("manifest.json");
        let text = fs::read_to_string(&manifest_path)?;
        let mut manifest: RunManifest = serde_json::from_str(&text)?;
        manifest.exit_code = exit_code;
        self.write_json("manifest.json", &manifest)
    }

    pub fn write_text(&self, name: &str, text: &str) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::write(&path, text)?;
        Ok(path)
    }

    fn write_json<T: Serialize + ?Sized>(&self, name: &str, value: &T) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(name);
        fs::write(path, serde_json::to_vec_pretty(value)?)?;
        Ok(())
    }

    fn append_jsonl<T: Serialize>(&self, name: &str, values: &[T]) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(name);
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        for value in values {
            file.write_all(serde_json::to_string(value)?.as_bytes())?;
            file.write_all(b"\n")?;
        }
        Ok(())
    }

    fn read_jsonl<T: for<'de> Deserialize<'de>>(&self, name: &str) -> Result<Vec<T>> {
        let path = self.root.join(name);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(path)?;
        let mut values = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            values.push(serde_json::from_str(line)?);
        }
        Ok(values)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunArtifactPaths {
    pub manifest: PathBuf,
    pub samples: PathBuf,
    pub events: PathBuf,
    pub artifacts: PathBuf,
    pub findings: PathBuf,
    pub json_report: PathBuf,
    pub perfetto_trace: PathBuf,
}
