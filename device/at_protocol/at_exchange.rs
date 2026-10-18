//! Own one diagnostic command from its first write through its final reply.
use at_command::Command;
use at_command_writer::AtCommandWriter;
use at_reply_reader::{AtReplyReader, ReplyReadError};
use at_response::ResponseEvent;
use std::io::{self, Read, Write};
use std::time::Instant;

#[derive(Debug)]
pub enum ExchangeError {
    Write(io::Error),
    Reply(ReplyReadError),
    Closed,
}
enum Phase<T> {
    Sending(AtCommandWriter<T>, Command, Instant),
    Reading(AtReplyReader<T>),
}

/// Sends one command and reads its reply using the same owned connection.
/// `new` takes a semantic command, a synchronized nonblocking transport and one
/// deadline shared by sending and reading. Wire encoding and reply semantics
/// belong to the command interface. `poll(now)` advances one IO operation and returns a reply event
/// or None; callers schedule polls and process notifications. After Complete,
/// call `finish()` to recover the transport. An error drops the transport and
/// permanently closes this exchange; later polls return Closed. Drop also closes
/// an owned SerialPort. No connection recovery or automatic command retry occurs.
/// The next command still requires a synchronized connection; queued notifications
/// and device resets need a session owner. This is a diagnostic probe building block.
pub struct AtExchange<T> {
    phase: Option<Phase<T>>,
}
impl<T: Read + Write> AtExchange<T> {
    pub fn new(transport: T, command: Command, deadline: Instant) -> Self {
        let writer = AtCommandWriter::new(transport, command, deadline);
        Self {
            phase: Some(Phase::Sending(writer, command, deadline)),
        }
    }
    pub fn poll(&mut self, now: Instant) -> Result<Option<ResponseEvent>, ExchangeError> {
        let phase = self.phase.take().ok_or(ExchangeError::Closed)?;
        let (next, event) = match phase {
            Phase::Sending(mut writer, command, deadline) => {
                if writer.poll(now).map_err(ExchangeError::Write)? {
                    let transport = writer.finish().map_err(|_| ExchangeError::Closed)?;
                    let reader = AtReplyReader::new(transport, command, deadline);
                    (Phase::Reading(reader), None)
                } else {
                    (Phase::Sending(writer, command, deadline), None)
                }
            }
            Phase::Reading(mut reader) => {
                let event = reader.poll(now).map_err(ExchangeError::Reply)?;
                (Phase::Reading(reader), event)
            }
        };
        self.phase = Some(next);
        Ok(event)
    }
    /// Return the transport only after Complete; otherwise return this exchange.
    /// This method sends and reads nothing.
    pub fn finish(mut self) -> Result<T, Self> {
        match self.phase.take() {
            Some(Phase::Reading(reader)) => match reader.finish() {
                Ok(transport) => Ok(transport),
                Err(reader) => {
                    self.phase = Some(Phase::Reading(reader));
                    Err(self)
                }
            },
            phase => {
                self.phase = phase;
                Err(self)
            }
        }
    }
}
