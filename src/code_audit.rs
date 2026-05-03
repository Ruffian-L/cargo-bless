//! Static Rust code audit for suspicious complexity and brittle patterns.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};

const MAX_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum BullshitKind {
    FakeComplexity,
    CargoCult,
    OverEngineering,
    ArcAbuse,
    RwLockAbuse,
    SleepAbuse,
    UnwrapAbuse,
    DynTraitAbuse,
    CloneAbuse,
    MutexAbuse,
}

impl BullshitKind {
    fn label(self) -> &'static str {
        match self {
            Self::FakeComplexity => "fake complexity",
            Self::CargoCult => "cargo cult",
            Self::OverEngineering => "over-engineering",
            Self::ArcAbuse => "Arc abuse",
            Self::RwLockAbuse => "RwLock abuse",
            Self::SleepAbuse => "sleep abuse",
            Self::UnwrapAbuse => "unwrap abuse",
            Self::DynTraitAbuse => "dyn trait abuse",
            Self::CloneAbuse => "clone abuse",
            Self::MutexAbuse => "mutex abuse",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BullshitAlert {
    pub kind: BullshitKind,
    pub confidence: f32,
    pub severity: f32,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub context_snippet: String,
    pub why_bs: String,
    pub suggestion: String,
}

#[derive(Debug, Clone)]
pub struct CodeAuditConfig {
    pub confidence_threshold: f32,
    pub max_file_bytes: u64,
    pub ignore_paths: Vec<String>,
    pub ignore_kinds: HashSet<String>,
}

impl Default for CodeAuditConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.60,
            max_file_bytes: MAX_FILE_BYTES,
            ignore_paths: Vec::new(),
            ignore_kinds: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAuditReport {
    pub files_scanned: usize,
    pub alerts: Vec<BullshitAlert>,
}

impl CodeAuditReport {
    pub fn is_clean(&self) -> bool {
        self.alerts.is_empty()
    }
}

/// Concatenate workspace member audits into one report (sums `files_scanned`, merges alerts).
pub fn merge_reports(reports: Vec<CodeAuditReport>) -> CodeAuditReport {
    let mut files_scanned = 0usize;
    let mut alerts = Vec::new();
    for r in reports {
        files_scanned += r.files_scanned;
        alerts.extend(r.alerts);
    }
    CodeAuditReport {
        files_scanned,
        alerts,
    }
}

pub fn scan_project(
    manifest_path: Option<&Path>,
    config: &CodeAuditConfig,
) -> Result<CodeAuditReport> {
    scan_project_with_filter(manifest_path, config, None)
}

pub fn scan_git_diff(
    manifest_path: Option<&Path>,
    config: &CodeAuditConfig,
) -> Result<CodeAuditReport> {
    let base_dir = project_base_dir(manifest_path);
    let filter = DiffFilter::from_git_diff(base_dir)?;
    scan_project_with_filter(manifest_path, config, Some(&filter))
}

fn scan_project_with_filter(
    manifest_path: Option<&Path>,
    config: &CodeAuditConfig,
    diff_filter: Option<&DiffFilter>,
) -> Result<CodeAuditReport> {
    let base_dir = manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut files = Vec::new();
    for dir in ["src", "tests", "examples", "benches"] {
        collect_rust_files(&base_dir.join(dir), config, &mut files)?;
    }

    let mut alerts = Vec::new();
    for file in &files {
        if is_ignored_path(file, config) {
            continue;
        }
        let code = fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let mut file_alerts = scan_code(&code, file, config)?;
        if let Some(filter) = diff_filter {
            file_alerts.retain(|alert| filter.includes(alert));
        }
        alerts.extend(file_alerts);
    }

    alerts.sort_by(|a, b| {
        b.severity
            .partial_cmp(&a.severity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    Ok(CodeAuditReport {
        files_scanned: files.len(),
        alerts,
    })
}

pub fn scan_code(
    code: &str,
    file: impl Into<PathBuf>,
    config: &CodeAuditConfig,
) -> Result<Vec<BullshitAlert>> {
    let file = file.into();
    if is_ignored_path(&file, config) {
        return Ok(Vec::new());
    }

    let ignored_ranges = parse_ignored_ranges(code).unwrap_or_default();
    let masked = mask_ranges(code, &ignored_ranges);
    let mut alerts = Vec::new();

    scan_regex_patterns(&masked, &file, &mut alerts)?;
    scan_line_patterns(&masked, &file, &mut alerts);
    scan_function_complexity(&masked, &file, &mut alerts);

    alerts.retain(|alert| alert.confidence >= config.confidence_threshold);
    alerts.retain(|alert| !config.ignore_kinds.contains(&format!("{:?}", alert.kind)));
    dedupe_alerts(&mut alerts);
    Ok(alerts)
}

pub fn config_from_policy(policy: Option<&crate::policy::Policy>) -> CodeAuditConfig {
    let mut config = CodeAuditConfig::default();
    if let Some(policy) = policy {
        config.ignore_paths = policy.code_audit.ignore_paths.clone();
        config.ignore_kinds = policy.code_audit.ignore_kinds.iter().cloned().collect();
    }
    config
}

fn project_base_dir(manifest_path: Option<&Path>) -> &Path {
    manifest_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn is_ignored_path(path: &Path, config: &CodeAuditConfig) -> bool {
    let path = path.to_string_lossy();
    config
        .ignore_paths
        .iter()
        .any(|pattern| path.contains(pattern))
}

fn collect_rust_files(
    dir: &Path,
    config: &CodeAuditConfig,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if path.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            collect_rust_files(&path, config, files)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        let metadata = entry.metadata()?;
        if metadata.len() <= config.max_file_bytes {
            files.push(path);
        }
    }

    Ok(())
}

fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "target" | "vendor" | "node_modules" | "dist" | "build" | "third_party"
        )
}

#[derive(Debug)]
struct DiffFilter {
    base_dir: PathBuf,
    changed_lines: HashMap<PathBuf, Vec<(usize, usize)>>,
}

impl DiffFilter {
    fn from_git_diff(base_dir: &Path) -> Result<Self> {
        let output = Command::new("git")
            .arg("-C")
            .arg(base_dir)
            .arg("diff")
            .arg("HEAD")
            .arg("--unified=0")
            .arg("--")
            .output()
            .with_context(|| "failed to run git diff HEAD --unified=0")?;

        if !output.status.success() {
            bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
            changed_lines: parse_changed_lines(&String::from_utf8_lossy(&output.stdout)),
        })
    }

    fn includes(&self, alert: &BullshitAlert) -> bool {
        let path = alert
            .file
            .strip_prefix(&self.base_dir)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| alert.file.clone());
        let path = normalize_diff_path(&path);
        self.changed_lines.get(&path).is_some_and(|ranges| {
            ranges
                .iter()
                .any(|(start, end)| alert.line >= *start && alert.line <= *end)
        })
    }
}

fn parse_changed_lines(diff: &str) -> HashMap<PathBuf, Vec<(usize, usize)>> {
    let mut current_file: Option<PathBuf> = None;
    let mut changed = HashMap::<PathBuf, Vec<(usize, usize)>>::new();

    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = Some(PathBuf::from(path));
            continue;
        }
        if line.starts_with("+++ /dev/null") {
            current_file = None;
            continue;
        }

        if let (Some(file), Some(range)) = (current_file.as_ref(), parse_hunk_new_range(line)) {
            changed.entry(file.clone()).or_default().push(range);
        }
    }

    changed
}

fn parse_hunk_new_range(line: &str) -> Option<(usize, usize)> {
    let hunk = line.strip_prefix("@@ ")?;
    let plus = hunk.split_whitespace().find(|part| part.starts_with('+'))?;
    let plus = plus.trim_start_matches('+');
    let (start, count) = plus
        .split_once(',')
        .map(|(start, count)| (start, count.parse::<usize>().ok()))
        .unwrap_or((plus, Some(1)));
    let start = start.parse::<usize>().ok()?;
    let count = count?;
    if count == 0 {
        None
    } else {
        Some((start, start + count - 1))
    }
}

fn normalize_diff_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn parse_ignored_ranges(code: &str) -> Result<Vec<(usize, usize)>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|err| anyhow::anyhow!("failed to load Rust tree-sitter grammar: {err}"))?;
    let tree = parser
        .parse(code, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse Rust source"))?;

    let mut ranges = Vec::new();
    collect_ignored_ranges(tree.root_node(), &mut ranges);
    Ok(ranges)
}

fn collect_ignored_ranges(node: Node<'_>, ranges: &mut Vec<(usize, usize)>) {
    if is_ignored_node(node.kind()) {
        ranges.push((node.start_byte(), node.end_byte()));
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ignored_ranges(child, ranges);
    }
}

fn is_ignored_node(kind: &str) -> bool {
    matches!(
        kind,
        "line_comment" | "block_comment" | "string_literal" | "raw_string_literal" | "char_literal"
    )
}

fn mask_ranges(code: &str, ranges: &[(usize, usize)]) -> String {
    let mut bytes = code.as_bytes().to_vec();
    for (start, end) in ranges {
        for idx in *start..*end {
            if let Some(byte) = bytes.get_mut(idx) {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| code.to_string())
}

fn scan_regex_patterns(code: &str, file: &Path, alerts: &mut Vec<BullshitAlert>) -> Result<()> {
    let patterns = [
        (
            r"Arc\s*<\s*RwLock\s*<",
            BullshitKind::OverEngineering,
            0.86,
            "Arc<RwLock<...>> is often shared mutable state wearing a tuxedo.",
            "Try explicit ownership, message passing, or a narrower shared state boundary.",
        ),
        (
            r"Arc\s*<\s*Mutex\s*<",
            BullshitKind::OverEngineering,
            0.82,
            "Arc<Mutex<...>> can be valid, but it is also a classic complexity magnet.",
            "Check whether ownership can stay local or the locked data can be smaller.",
        ),
        (
            r"Mutex\s*<\s*HashMap\s*<",
            BullshitKind::MutexAbuse,
            0.76,
            "A Mutex<HashMap<...>> is a blunt concurrency primitive.",
            "Consider sharding, DashMap, or reducing shared mutable state.",
        ),
        (
            r"RwLock\s*<",
            BullshitKind::RwLockAbuse,
            0.64,
            "RwLock adds coordination cost and can hide unclear ownership.",
            "Use it only when read-heavy sharing is real and measured.",
        ),
        (
            r"\b(std::thread::sleep|tokio::time::sleep)\s*\(",
            BullshitKind::SleepAbuse,
            0.78,
            "Sleep calls are often timing bullshit instead of synchronization.",
            "Replace sleeps with explicit readiness, timeouts, retries, or test clocks.",
        ),
        (
            r"Arc\s*<\s*(String|Vec\s*<|Box\s*<)",
            BullshitKind::ArcAbuse,
            0.62,
            "Arc<String>, Arc<Vec<...>>, or Arc<Box<...>> wraps a value type in shared ownership — often unnecessary.",
            "Use Arc<str> instead of Arc<String>, or reconsider whether sharing is needed at all.",
        ),
    ];

    for (pattern, kind, confidence, why, suggestion) in patterns {
        let regex = Regex::new(pattern)?;
        for mat in regex.find_iter(code) {
            alerts.push(make_alert(
                kind,
                confidence,
                file,
                code,
                mat.start(),
                mat.end(),
                why,
                suggestion,
            ));
        }
    }

    Ok(())
}

fn scan_line_patterns(code: &str, file: &Path, alerts: &mut Vec<BullshitAlert>) {
    for (line_idx, line) in code.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(col) = line.find(".unwrap()") {
            alerts.push(alert_from_line(
                BullshitKind::UnwrapAbuse,
                0.72,
                file,
                line_idx + 1,
                col + 1,
                line,
                "unwrap() is a runtime trap dressed up as confidence.",
                "Propagate the error with ?, add context, or handle the failure explicitly.",
            ));
        }

        let clone_count = line.matches(".clone()").count();
        if clone_count >= 2 {
            alerts.push(alert_from_line(
                BullshitKind::CloneAbuse,
                (0.60 + clone_count as f32 * 0.08).min(0.92),
                file,
                line_idx + 1,
                line.find(".clone()").unwrap_or(0) + 1,
                line,
                "Multiple clone() calls on one line can hide ownership confusion.",
                "Check whether borrowing, moving, or restructuring removes the copies.",
            ));
        }

        let dyn_count = trimmed.matches("dyn ").count();
        if dyn_count >= 3 {
            alerts.push(alert_from_line(
                BullshitKind::DynTraitAbuse,
                0.80,
                file,
                line_idx + 1,
                line.find("dyn ").unwrap_or(0) + 1,
                line,
                "Heavy dyn usage may be abstraction theater.",
                "Prefer concrete types or generics unless runtime polymorphism is needed.",
            ));
        }

        if trimmed.starts_with("use std::collections::{")
            && trimmed.contains("HashMap")
            && trimmed.contains("BTreeMap")
        {
            alerts.push(alert_from_line(
                BullshitKind::CargoCult,
                0.62,
                file,
                line_idx + 1,
                line.find("HashMap").unwrap_or(0) + 1,
                line,
                "Broad collection imports can signal cargo-cult scaffolding.",
                "Import the collection you actually use, or qualify rare uses inline.",
            ));
        }
    }
}

fn scan_function_complexity(code: &str, file: &Path, alerts: &mut Vec<BullshitAlert>) {
    let lines: Vec<&str> = code.lines().collect();
    let mut idx = 0;

    while idx < lines.len() {
        let line = lines[idx];
        if !looks_like_fn_start(line) {
            idx += 1;
            continue;
        }

        let start_line = idx + 1;
        let mut brace_balance = 0isize;
        let mut saw_body = false;
        let mut complexity = 0usize;
        let mut end_idx = idx;

        while end_idx < lines.len() {
            let current = lines[end_idx];
            complexity += line_complexity(current);
            for ch in current.chars() {
                if ch == '{' {
                    saw_body = true;
                    brace_balance += 1;
                } else if ch == '}' {
                    brace_balance -= 1;
                }
            }
            if saw_body && brace_balance <= 0 {
                break;
            }
            end_idx += 1;
        }

        if saw_body && complexity >= 6 {
            let confidence = (complexity as f32 / 24.0).clamp(0.66, 0.95);
            alerts.push(alert_from_line(
                BullshitKind::FakeComplexity,
                confidence,
                file,
                start_line,
                line.find("fn").unwrap_or(0) + 1,
                line,
                &format!(
                    "Function complexity score is {complexity}; this smells like fake complexity."
                ),
                "Split the function around decisions, loops, and side effects.",
            ));
        }

        idx = end_idx.saturating_add(1);
    }
}

fn looks_like_fn_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("async fn ")
        || trimmed.starts_with("pub async fn ")
}

fn line_complexity(line: &str) -> usize {
    let mut score = 0;
    let trimmed = line.trim_start();
    for token in [
        "if ", "if(", "match ", "for ", "while ", "loop ", "&&", "||",
    ] {
        score += line.matches(token).count();
    }
    if trimmed.starts_with("if(") {
        score += 1;
    }
    score += line.matches("?;").count();
    score += line.matches(".unwrap()").count() * 2;
    score
}

#[allow(clippy::too_many_arguments)]
fn make_alert(
    kind: BullshitKind,
    confidence: f32,
    file: &Path,
    code: &str,
    start: usize,
    end: usize,
    why_bs: &str,
    suggestion: &str,
) -> BullshitAlert {
    let (line, column) = line_column(code, start);
    BullshitAlert {
        kind,
        confidence,
        severity: confidence,
        file: file.to_path_buf(),
        line,
        column,
        context_snippet: snippet(code, start, end),
        why_bs: why_bs.to_string(),
        suggestion: suggestion.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn alert_from_line(
    kind: BullshitKind,
    confidence: f32,
    file: &Path,
    line: usize,
    column: usize,
    context: &str,
    why_bs: &str,
    suggestion: &str,
) -> BullshitAlert {
    BullshitAlert {
        kind,
        confidence,
        severity: confidence,
        file: file.to_path_buf(),
        line,
        column,
        context_snippet: context.trim().to_string(),
        why_bs: why_bs.to_string(),
        suggestion: suggestion.to_string(),
    }
}

fn line_column(code: &str, byte_pos: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;

    for (idx, ch) in code.char_indices() {
        if idx >= byte_pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

fn snippet(code: &str, start: usize, end: usize) -> String {
    let line_start = code[..start].rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = code[end..].find('\n').map_or(code.len(), |idx| end + idx);
    code[line_start..line_end].trim().to_string()
}

fn dedupe_alerts(alerts: &mut Vec<BullshitAlert>) {
    alerts.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
            .then_with(|| format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)))
    });
    alerts.dedup_by(|a, b| {
        a.file == b.file && a.line == b.line && a.column == b.column && a.kind == b.kind
    });
}

pub fn kind_label(kind: BullshitKind) -> &'static str {
    kind.label()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> CodeAuditConfig {
        CodeAuditConfig::default()
    }

    #[test]
    fn detects_unwrap_and_sleep() {
        let code = r#"
fn main() {
    let value = thing().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
}
"#;
        let alerts = scan_code(code, "src/main.rs", &config()).unwrap();
        assert!(alerts.iter().any(|a| a.kind == BullshitKind::UnwrapAbuse));
        assert!(alerts.iter().any(|a| a.kind == BullshitKind::SleepAbuse));
    }

    #[test]
    fn detects_shared_mutable_state() {
        let code = "type Store = Arc<RwLock<HashMap<String, String>>>;";
        let alerts = scan_code(code, "src/lib.rs", &config()).unwrap();
        assert!(alerts
            .iter()
            .any(|a| a.kind == BullshitKind::OverEngineering));
    }

    #[test]
    fn detects_fake_complexity() {
        let code = r#"
fn tangled(x: usize) -> usize {
    if x > 1 { if x > 2 { if x > 3 { if x > 4 { if x > 5 { return x; }}}}}
    match x { 0 => 1, 1 => 2, _ => 3 }
}
"#;
        let alerts = scan_code(code, "src/lib.rs", &config()).unwrap();
        assert!(alerts
            .iter()
            .any(|a| a.kind == BullshitKind::FakeComplexity));
    }

    #[test]
    fn ignores_patterns_in_strings_and_comments() {
        let code = r#"
fn main() {
    let text = "Arc<RwLock<HashMap<String, String>>> and thing().unwrap()";
    // std::thread::sleep(std::time::Duration::from_millis(10));
}
"#;
        let alerts = scan_code(code, "src/main.rs", &config()).unwrap();
        assert!(
            alerts.is_empty(),
            "strings/comments should not produce bullshit alerts: {alerts:?}"
        );
    }

    #[test]
    fn policy_suppresses_kind_and_path() {
        let mut cfg = config();
        cfg.ignore_kinds.insert("UnwrapAbuse".to_string());
        let alerts = scan_code("fn main() { thing().unwrap(); }", "src/main.rs", &cfg).unwrap();
        assert!(alerts.is_empty());

        let mut cfg = config();
        cfg.ignore_paths.push("generated".to_string());
        let alerts = scan_code(
            "fn main() { thing().unwrap(); }",
            "src/generated/main.rs",
            &cfg,
        )
        .unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn parses_diff_changed_ranges() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 111..222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,0 +2,3 @@
+fn main() {
+    thing().unwrap();
+}
"#;
        let changed = parse_changed_lines(diff);
        assert_eq!(changed.get(Path::new("src/main.rs")), Some(&vec![(2, 4)]));
    }
}
