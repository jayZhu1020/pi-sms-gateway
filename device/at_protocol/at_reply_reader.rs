//! Read one diagnostic command reply from an already-synchronized connection.
use at_command::Command;
use at_decoder::{AtLineDecoder, DecodeError};
use at_response::{AtResponse, ResponseError, ResponseEvent};
use std::io::{self, Read};
use std::time::Instant;

#[derive(Debug)]
pub enum ReplyReadError {
    Io(io::Error),
    Decode(DecodeError),
    Response(ResponseError),
    Closed,
}

/// Reads modem bytes and returns complete reply or notification events.
/// `new` owns a nonblocking transport after a command was sent. Supply the same
/// semantic command and a deadline; wire spellings and reply decoding stay hidden.
/// `poll(now)` reads at most one byte, returning an event when a line completes,
/// or None for partial lines/temporary IO errors. Poll during silence for timeout.
/// Complete ends reading; IO, decoding and response errors stop it permanently.
/// `finish()` returns the transport only after Complete, without reading more.
/// Reading one byte avoids consuming notifications after the final OK; callers
/// must schedule repeated polls. No buffering beyond the decoder's bounded line,
/// sleeping, command sending or recovery occurs here. Use only a synchronized
/// stream: constructing a fresh reader cannot distinguish an old command's OK.
pub struct AtReplyReader<R> {
    transport: R,
    decoder: AtLineDecoder,
    response: AtResponse,
    stopped: bool,
    complete: bool,
}
impl<R: Read> AtReplyReader<R> {
    pub fn new(transport: R, command: Command, deadline: Instant) -> Self {
        Self {
            transport,
            decoder: AtLineDecoder::default(),
            response: AtResponse::new(command, deadline),
            stopped: false,
            complete: false,
        }
    }
    pub fn poll(&mut self, now: Instant) -> Result<Option<ResponseEvent>, ReplyReadError> {
        if self.stopped {
            return Err(ReplyReadError::Closed);
        }
        self.stopped = true;
        self.response
            .check_deadline(now)
            .map_err(ReplyReadError::Response)?;
        let mut byte = [0];
        let line = match self.transport.read(&mut byte) {
            Ok(0) => return Err(ReplyReadError::Io(io::ErrorKind::UnexpectedEof.into())),
            Ok(_) => self.decoder.push(byte[0]).map_err(ReplyReadError::Decode)?,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                None
            }
            Err(e) => return Err(ReplyReadError::Io(e)),
        };
        let event = line
            .map(|line| self.response.accept(line, now))
            .transpose()
            .map_err(ReplyReadError::Response)?;
        self.complete = matches!(event, Some(ResponseEvent::Complete));
        self.stopped = self.complete;
        Ok(event)
    }
    /// Return the transport after a successful reply; otherwise return this reader.
    pub fn finish(self) -> Result<R, Self> {
        if self.complete {
            Ok(self.transport)
        } else {
            Err(self)
        }
    }
}
