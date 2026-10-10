//! Incremental framing for ASCII AT control/PDU-mode traffic, not SMS text mode.

const MAX_LINE_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    InvalidByte,
    LineTooLong,
    Failed,
}

/// Joins incoming bytes into lines and returns them in the order received.
/// Keeps modem event messages too, even when no command asked for them.
/// Ignores blank lines. A line ends with LF (`\n`), optionally preceded by
/// one or more CR (`\r`) bytes; those bytes may arrive in separate reads.
///
/// Invalid input or a line over 4096 bytes permanently stops this decoder.
/// The caller must recover the serial connection before using a new decoder.
/// Otherwise, a delayed `OK` from an old command could look like the reply
/// to a new command. This decoder does not perform that recovery itself.
///
/// We do not derive Rust's `Debug` printing trait here: buffered bytes may
/// contain private message data that should not appear in diagnostic logs.
#[derive(Default)]
pub struct AtLineDecoder {
    pending: Vec<u8>,
    carriage_return: bool,
    failed: bool,
}

impl AtLineDecoder {
    /// Pass in one received byte. Returns `Some(line)` when a full line is ready.
    /// Reading the port, matching replies to commands, and timeouts happen elsewhere.
    pub fn push(&mut self, byte: u8) -> Result<Option<String>, DecodeError> {
        if self.failed {
            return Err(DecodeError::Failed);
        }
        let result = self.accept(byte);
        if result.is_err() {
            self.failed = true;
            self.pending.clear();
        }
        result
    }

    fn accept(&mut self, byte: u8) -> Result<Option<String>, DecodeError> {
        if self.carriage_return && byte != b'\n' && byte != b'\r' {
            return Err(DecodeError::InvalidByte);
        }
        match byte {
            b'\r' => self.carriage_return = true,
            b'\n' => {
                self.carriage_return = false;
                if !self.pending.is_empty() {
                    let bytes = std::mem::take(&mut self.pending);
                    return String::from_utf8(bytes)
                        .map(Some)
                        .map_err(|_| DecodeError::InvalidByte);
                }
            }
            b'\t' | b' '..=b'~' => {
                if self.pending.len() == MAX_LINE_BYTES {
                    return Err(DecodeError::LineTooLong);
                }
                self.pending.push(byte);
            }
            _ => return Err(DecodeError::InvalidByte),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(decoder: &mut AtLineDecoder, bytes: &[u8]) -> Vec<String> {
        bytes
            .iter()
            .filter_map(|&b| decoder.push(b).unwrap())
            .collect()
    }

    #[test]
    fn fragmented_response_preserves_unsolicited_lines() {
        let mut decoder = AtLineDecoder::default();
        assert!(read(&mut decoder, b"\r\r\n+CP").is_empty());
        assert_eq!(read(&mut decoder, b"IN: READY\r"), Vec::<String>::new());
        assert_eq!(
            read(&mut decoder, b"\n+CMTI: \"SM\",7\r\nOK\r\n"),
            vec!["+CPIN: READY", "+CMTI: \"SM\",7", "OK"]
        );
    }

    #[test]
    fn coalesced_lines_and_partial_tail_are_preserved() {
        let mut decoder = AtLineDecoder::default();
        assert_eq!(read(&mut decoder, b"AT\n\nERROR\n+CM"), vec!["AT", "ERROR"]);
        assert_eq!(
            read(&mut decoder, b"S ERROR: 500\n"),
            vec!["+CMS ERROR: 500"]
        );
    }

    #[test]
    fn malformed_input_poisons_session() {
        for bytes in [b"bad\0".as_slice(), b"\xff", b"\rX"] {
            let mut decoder = AtLineDecoder::default();
            for &byte in &bytes[..bytes.len() - 1] {
                assert!(decoder.push(byte).is_ok());
            }
            assert_eq!(
                decoder.push(bytes[bytes.len() - 1]),
                Err(DecodeError::InvalidByte)
            );
            assert_eq!(decoder.push(b'\n'), Err(DecodeError::Failed));
        }
    }

    #[test]
    fn maximum_line_is_accepted_but_overflow_poisons_session() {
        let mut decoder = AtLineDecoder::default();
        assert!(read(&mut decoder, &vec![b'A'; MAX_LINE_BYTES]).is_empty());
        assert_eq!(decoder.push(b'\n').unwrap().unwrap().len(), MAX_LINE_BYTES);
        read(&mut decoder, &vec![b'A'; MAX_LINE_BYTES]);
        assert_eq!(decoder.push(b'B'), Err(DecodeError::LineTooLong));
        assert_eq!(decoder.push(b'\n'), Err(DecodeError::Failed));
    }
}
