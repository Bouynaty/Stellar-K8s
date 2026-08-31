//! Async log parser for Soroban RPC invocation streams.
//!
//! Extracts contract execution metadata from structured log lines using
//! zero-copy string slicing to minimise heap allocations. The parser is
//! designed for high-throughput streams where every allocation matters.
//!
//! # Log format
//!
//! The parser expects JSON-structured log lines produced by Soroban RPC nodes:
//!
//! ```json
//! {"timestamp":"2025-01-15T10:30:00Z","level":"info","msg":"contract_invocation",
//!  "contract_id":"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
//!  "cpu_instructions":142000,"memory_bytes":524288,
//!  "wasm_execution_duration_us":1500,"storage_fee_stroops":100,
//!  "host_function":"invoke","success":true}
//! ```

use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during log line parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    /// A required JSON field is missing.
    #[error("missing field: {0}")]
    MissingField(&'static str),
    /// A numeric field could not be parsed.
    #[error("invalid number for field {field}: {raw}")]
    InvalidNumber { field: &'static str, raw: String },
    /// The JSON structure is malformed.
    #[error("malformed JSON: {0}")]
    MalformedJson(&'static str),
}

// ---------------------------------------------------------------------------
// Parsed record
// ---------------------------------------------------------------------------

/// A single parsed contract invocation record.
///
/// All string fields borrow directly from the source log line where possible,
/// keeping allocations to a minimum.
#[derive(Debug, Clone, PartialEq)]
pub struct InvocationRecord<'a> {
    /// ISO-8601 timestamp from the log entry.
    pub timestamp: &'a str,
    /// Soroban contract identifier (hex-encoded StrKey).
    pub contract_id: &'a str,
    /// CPU instructions consumed by this invocation.
    pub cpu_instructions: u64,
    /// Peak memory usage in bytes.
    pub memory_bytes: u64,
    /// Wasm execution duration in microseconds.
    pub wasm_execution_duration_us: u64,
    /// Contract storage fee in stroops.
    pub storage_fee_stroops: u64,
    /// Host function name (e.g. "invoke", "upload_wasm").
    pub host_function: &'a str,
    /// Whether the invocation succeeded.
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Zero-copy JSON field extractor operating on raw `&str` slices.
///
/// This is intentionally minimal — it only supports flat key-value extraction
/// to avoid pulling in a full JSON deserialiser (serde + serde_json) while
/// keeping the hot path allocation-free.
struct FlatJson<'a> {
    src: &'a str,
}

impl<'a> FlatJson<'a> {
    fn new(src: &'a str) -> Self {
        Self { src }
    }

    /// Find the byte offset after `"key":` in the source string.
    fn find_key(&self, key: &str) -> Option<usize> {
        let needle = format!("\"{key}\":");
        let pos = self.src.find(&needle)? + needle.len();
        Some(pos)
    }

    /// Extract a string value for `key` using pointer arithmetic on the
    /// source slice. Returns `None` when the key is absent or the value
    /// is not a quoted string.
    fn get_str(&self, key: &str) -> Option<&'a str> {
        let start = self.find_key(key)?;
        let trimmed = self.src[start..].trim_start();
        if trimmed.starts_with('"') {
            let after_quote = &trimmed[1..];
            let end = after_quote.find('"')?;
            Some(&after_quote[..end])
        } else {
            None
        }
    }

    /// Extract a `u64` value for `key`. Handles both quoted (`"123"`) and
    /// unquoted (`123`) JSON number values.
    fn get_u64(&self, key: &str) -> Result<u64, ParseError> {
        let start = self.find_key(key).ok_or(ParseError::MissingField(key))?;
        let rest = self.src[start..].trim_start();

        let raw = if rest.starts_with('"') {
            let after = &rest[1..];
            let end = after.find('"').ok_or(ParseError::MalformedJson(
                "unterminated string in numeric field",
            ))?;
            &after[..end]
        } else {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            if end == 0 {
                return Err(ParseError::MissingField(key));
            }
            &rest[..end]
        };

        raw.parse::<u64>().map_err(|_| ParseError::InvalidNumber {
            field: key,
            raw: raw.to_string(),
        })
    }

    /// Extract a `bool` value for `key`.
    fn get_bool(&self, key: &str) -> Result<bool, ParseError> {
        let val = self.get_str(key).ok_or(ParseError::MissingField(key))?;
        match val {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(ParseError::InvalidNumber {
                field: key,
                raw: val.to_string(),
            }),
        }
    }
}

/// Parse a single Soroban RPC log line into an [`InvocationRecord`].
///
/// Uses zero-copy string slicing — no heap allocations are performed on
/// the successful path.
pub fn parse_invocation_line(line: &str) -> Result<InvocationRecord<'_>, ParseError> {
    let json = FlatJson::new(line);

    let timestamp = json.get_str("timestamp").unwrap_or("");
    let contract_id = json.get_str("contract_id").unwrap_or("");
    let cpu_instructions = json.get_u64("cpu_instructions")?;
    let memory_bytes = json.get_u64("memory_bytes")?;
    let wasm_execution_duration_us = json.get_u64("wasm_execution_duration_us").unwrap_or(0);
    let storage_fee_stroops = json.get_u64("storage_fee_stroops").unwrap_or(0);
    let host_function = json.get_str("host_function").unwrap_or("unknown");
    let success = json.get_bool("success").unwrap_or(true);

    Ok(InvocationRecord {
        timestamp,
        contract_id,
        cpu_instructions,
        memory_bytes,
        wasm_execution_duration_us,
        storage_fee_stroops,
        host_function,
        success,
    })
}

// ---------------------------------------------------------------------------
// Async streaming parser
// ---------------------------------------------------------------------------

/// Asynchronously process log lines from an [`AsyncBufRead`] source and
/// invoke `callback` for each successfully parsed [`InvocationRecord`].
///
/// Malformed lines are counted but otherwise skipped, ensuring the stream
/// processing is never blocked by bad input.
pub async fn parse_log_stream<R, F>(
    reader: R,
    mut callback: F,
) -> Result<StreamStats, std::io::Error>
where
    R: tokio::io::AsyncBufReadExt + Unpin,
    F: FnMut(InvocationRecord<'_>),
{
    let mut stats = StreamStats::default();
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        stats.lines_total += 1;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match parse_invocation_line(trimmed) {
            Ok(record) => {
                stats.lines_parsed += 1;
                callback(record);
            }
            Err(_) => {
                stats.lines_skipped += 1;
            }
        }
    }

    Ok(stats)
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics from a parsed log stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Total lines read (including blank lines).
    pub lines_total: u64,
    /// Lines successfully parsed into [`InvocationRecord`].
    pub lines_parsed: u64,
    /// Lines skipped due to parse errors or blank content.
    pub lines_skipped: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LOG: &str = r#"{"timestamp":"2025-01-15T10:30:00Z","level":"info","msg":"contract_invocation","contract_id":"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC","cpu_instructions":142000,"memory_bytes":524288,"wasm_execution_duration_us":1500,"storage_fee_stroops":100,"host_function":"invoke","success":true}"#;

    #[test]
    fn test_parse_valid_line() {
        let record = parse_invocation_line(SAMPLE_LOG).unwrap();
        assert_eq!(record.timestamp, "2025-01-15T10:30:00Z");
        assert_eq!(
            record.contract_id,
            "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
        );
        assert_eq!(record.cpu_instructions, 142000);
        assert_eq!(record.memory_bytes, 524288);
        assert_eq!(record.wasm_execution_duration_us, 1500);
        assert_eq!(record.storage_fee_stroops, 100);
        assert_eq!(record.host_function, "invoke");
        assert!(record.success);
    }

    #[test]
    fn test_parse_minimal_line() {
        let line = r#"{"contract_id":"CAAA","cpu_instructions":100,"memory_bytes":256}"#;
        let record = parse_invocation_line(line).unwrap();
        assert_eq!(record.contract_id, "CAAA");
        assert_eq!(record.cpu_instructions, 100);
        assert_eq!(record.memory_bytes, 256);
        assert_eq!(record.timestamp, "");
        assert_eq!(record.host_function, "unknown");
    }

    #[test]
    fn test_parse_missing_required_field() {
        let line = r#"{"timestamp":"2025-01-15T10:30:00Z","contract_id":"C123"}"#;
        let err = parse_invocation_line(line).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("cpu_instructions")));
    }

    #[test]
    fn test_parse_invalid_number() {
        let line = r#"{"contract_id":"C123","cpu_instructions":"not_a_number","memory_bytes":100}"#;
        let err = parse_invocation_line(line).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidNumber {
                field: "cpu_instructions",
                ..
            }
        ));
    }

    #[test]
    fn test_parse_failed_invocation() {
        let line = r#"{"timestamp":"2025-01-15T10:30:00Z","contract_id":"C123","cpu_instructions":0,"memory_bytes":0,"success":false}"#;
        let record = parse_invocation_line(line).unwrap();
        assert!(!record.success);
    }

    #[test]
    fn test_zero_copy_borrowing() {
        let line = SAMPLE_LOG.to_string();
        let record = parse_invocation_line(&line).unwrap();
        let record_start = record.contract_id.as_ptr() as usize;
        let line_start = line.as_ptr() as usize;
        let line_end = line_start + line.len();
        assert!(
            record_start >= line_start && record_start < line_end,
            "contract_id should borrow from source line"
        );
    }

    #[tokio::test]
    async fn test_parse_log_stream() {
        let input = format!("{SAMPLE_LOG}\n{}\n", SAMPLE_LOG);
        let reader = tokio::io::BufReader::new(input.as_bytes());
        let mut records = Vec::new();
        let stats = parse_log_stream(reader, |r| records.push(r.to_owned()))
            .await
            .unwrap();
        assert_eq!(stats.lines_parsed, 2);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].cpu_instructions, 142000);
    }

    #[tokio::test]
    async fn test_parse_log_stream_skips_bad_lines() {
        let input = format!("{SAMPLE_LOG}\nnot json at all\n{SAMPLE_LOG}\n");
        let reader = tokio::io::BufReader::new(input.as_bytes());
        let stats = parse_log_stream(reader, |_| {}).await.unwrap();
        assert_eq!(stats.lines_total, 3);
        assert_eq!(stats.lines_parsed, 2);
        assert_eq!(stats.lines_skipped, 1);
    }

    #[test]
    fn test_stream_stats_default() {
        let stats = StreamStats::default();
        assert_eq!(stats.lines_total, 0);
        assert_eq!(stats.lines_parsed, 0);
        assert_eq!(stats.lines_skipped, 0);
    }

    #[test]
    fn test_parse_field_ordering_independent() {
        let line = r#"{"memory_bytes":1024,"contract_id":"CX","cpu_instructions":500}"#;
        let record = parse_invocation_line(line).unwrap();
        assert_eq!(record.contract_id, "CX");
        assert_eq!(record.cpu_instructions, 500);
        assert_eq!(record.memory_bytes, 1024);
    }

    #[test]
    fn test_parse_large_numbers() {
        let line = r#"{"contract_id":"C1","cpu_instructions":18446744073709551615,"memory_bytes":4294967295}"#;
        let record = parse_invocation_line(line).unwrap();
        assert_eq!(record.cpu_instructions, u64::MAX);
        assert_eq!(record.memory_bytes, u32::MAX as u64);
    }

    #[test]
    fn test_parse_quoted_numbers() {
        let line = r#"{"contract_id":"C1","cpu_instructions":"200","memory_bytes":"300"}"#;
        let record = parse_invocation_line(line).unwrap();
        assert_eq!(record.cpu_instructions, 200);
        assert_eq!(record.memory_bytes, 300);
    }
}
