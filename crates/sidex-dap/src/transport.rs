//! DAP transport layer using Content-Length framing (same as LSP).

use std::io::Write;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::Mutex;

use crate::protocol::DapMessage;

/// Bidirectional DAP transport over stdin/stdout of a child process.
pub struct DapTransport {
    writer: Mutex<ChildStdin>,
    reader: Mutex<BufReader<ChildStdout>>,
}

impl DapTransport {
    /// Creates a new transport from the child process's stdin and stdout.
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        Self {
            writer: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
        }
    }

    /// Sends a DAP message using Content-Length framing.
    pub async fn send(&self, message: &DapMessage) -> Result<()> {
        let body = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        let mut writer = self.writer.lock().await;
        writer.write_all(header.as_bytes()).await?;
        writer.write_all(body.as_bytes()).await?;
        writer.flush().await?;

        log::trace!("DAP send: {body}");
        Ok(())
    }

    /// Receives a DAP message, blocking until one arrives.
    pub async fn recv(&self) -> Result<DapMessage> {
        let mut reader = self.reader.lock().await;
        #[allow(clippy::explicit_auto_deref)]
        let content_length = read_content_length(&mut *reader).await?;
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).await?;

        let body = String::from_utf8(buf).context("DAP message not valid UTF-8")?;
        log::trace!("DAP recv: {body}");

        let message: DapMessage =
            serde_json::from_str(&body).context("failed to parse DAP message")?;
        Ok(message)
    }
}

/// Reads headers until we find Content-Length, consuming the blank line after.
async fn read_content_length(reader: &mut BufReader<ChildStdout>) -> Result<usize> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            bail!("DAP transport: unexpected EOF while reading headers");
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("invalid Content-Length value")?,
            );
        }
    }

    content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length header"))
}

/// Encodes a DAP message into a Content-Length framed byte buffer (sync, for testing).
pub fn encode_message(message: &DapMessage) -> Result<Vec<u8>> {
    let body = serde_json::to_string(message)?;
    let mut buf = Vec::new();
    write!(buf, "Content-Length: {}\r\n\r\n", body.len())?;
    buf.extend_from_slice(body.as_bytes());
    Ok(buf)
}

/// Decodes a single DAP message from a Content-Length framed byte slice (sync, for testing).
pub fn decode_message(data: &[u8]) -> Result<(DapMessage, usize)> {
    let text = std::str::from_utf8(data).context("not valid UTF-8")?;

    let header_end = text
        .find("\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("no header terminator found"))?;

    let headers = &text[..header_end];
    let mut content_length: Option<usize> = None;
    for line in headers.split("\r\n") {
        if let Some(val) = line.strip_prefix("Content-Length:") {
            content_length = Some(val.trim().parse()?);
        }
    }

    let content_length = content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length"))?;
    let body_start = header_end + 4; // skip \r\n\r\n
    let body_end = body_start + content_length;

    if data.len() < body_end {
        bail!(
            "incomplete message: need {body_end} bytes, got {}",
            data.len()
        );
    }

    let body = &text[body_start..body_end];
    let message: DapMessage = serde_json::from_str(body)?;
    Ok((message, body_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DapCommand, DapRequest};
    use serde_json::json;

    #[test]
    fn encode_decode_roundtrip() {
        let msg = DapMessage::Request(DapRequest::new(
            1,
            DapCommand::Initialize,
            json!({"clientID": "sidex"}),
        ));
        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());

        if let DapMessage::Request(req) = decoded {
            assert_eq!(req.seq, 1);
            assert_eq!(req.command, DapCommand::Initialize);
        } else {
            panic!("expected Request");
        }
    }

    #[test]
    fn content_length_framing_format() {
        let msg = DapMessage::Request(DapRequest::new(
            42,
            DapCommand::Threads,
            serde_json::Value::Null,
        ));
        let encoded = encode_message(&msg).unwrap();
        let text = std::str::from_utf8(&encoded).unwrap();
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));
    }

    #[test]
    fn decode_incomplete_fails() {
        let result = decode_message(b"Content-Length: 100\r\n\r\nshort");
        assert!(result.is_err());
    }

    #[test]
    fn decode_missing_header_fails() {
        let result = decode_message(b"SomethingElse: 10\r\n\r\n{}");
        assert!(result.is_err());
    }

    #[test]
    fn multiple_messages_in_stream() {
        let msg1 = DapMessage::Request(DapRequest::new(1, DapCommand::Initialize, json!({})));
        let msg2 = DapMessage::Request(DapRequest::new(2, DapCommand::Threads, json!(null)));

        let mut stream = encode_message(&msg1).unwrap();
        stream.extend_from_slice(&encode_message(&msg2).unwrap());

        let (decoded1, offset) = decode_message(&stream).unwrap();
        let (decoded2, _) = decode_message(&stream[offset..]).unwrap();

        if let DapMessage::Request(r) = decoded1 {
            assert_eq!(r.seq, 1);
        } else {
            panic!("expected Request");
        }
        if let DapMessage::Request(r) = decoded2 {
            assert_eq!(r.seq, 2);
        } else {
            panic!("expected Request");
        }
    }
}
