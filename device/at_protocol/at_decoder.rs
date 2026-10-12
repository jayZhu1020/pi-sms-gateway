//! Incremental framing for ASCII AT control/PDU-mode traffic, not SMS text mode.

mod at_constant;

const MAX_LINE_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    InvalidByte,
    LineTooLong,
    Failed,
}

/// Turns incoming modem bytes into complete text lines.
///
/// Create an empty decoder with `AtLineDecoder::default()`. Call `push(byte)`
/// once per received `u8`: `Ok(None)` means no nonempty line is ready;
/// `Ok(Some(line))` returns a completed `String` without its CR/LF ending.
/// Lines stay in arrival order, including notifications. Partial lines wait
/// for more bytes; blank lines are skipped. Rules are in `at_constant.rs`.
///
/// `push` returns `InvalidByte` for bad input and `LineTooLong` above 4096
/// text bytes. Either error stops this decoder; later calls return `Failed`.
/// Recover the serial connection before creating a replacement decoder.
/// This object does not read the port, match replies, or handle timeouts.
/// It has no `Debug` printing because buffered data may be private.
#[derive(Default)]
pub struct AtLineDecoder {
    pending: Vec<u8>,
    carriage_return: bool,
    failed: bool,
}

impl AtLineDecoder {
    /// Accept one byte; return a completed line, no line yet, or a decoding error.
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

    // Line-ending rules are documented in at_constant.rs.
    fn accept(&mut self, byte: u8) -> Result<Option<String>, DecodeError> {
        if self.carriage_return && byte != at_constant::LF && byte != at_constant::CR {
            return Err(DecodeError::InvalidByte);
        }
        match byte {
            at_constant::CR => self.carriage_return = true,
            at_constant::LF => {
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
            assert_eq!(decoder.push(at_constant::LF), Err(DecodeError::Failed));
        }
    }

    #[test]
    fn maximum_line_is_accepted_but_overflow_poisons_session() {
        let mut decoder = AtLineDecoder::default();
        assert!(read(&mut decoder, &vec![b'A'; MAX_LINE_BYTES]).is_empty());
        assert_eq!(
            decoder.push(at_constant::LF).unwrap().unwrap().len(),
            MAX_LINE_BYTES
        );
        read(&mut decoder, &vec![b'A'; MAX_LINE_BYTES]);
        assert_eq!(decoder.push(b'B'), Err(DecodeError::LineTooLong));
        assert_eq!(decoder.push(at_constant::LF), Err(DecodeError::Failed));
    }
}
