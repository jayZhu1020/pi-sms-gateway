//! Hardware-independent receive-SMS boundary. Adapters own transport and AT commands.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    Unsupported,
    Disconnected,
    InvalidHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub receive_sms: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceHealth {
    pub sim_ready: bool,
    pub registered: bool,
}

/// Adapter-scoped identity, valid until storage changes; never a message UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsHandle(pub String);

/// Raw SMS-DELIVER PDU bytes; decoding and durable intake are separate layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsSegment {
    pub handle: SmsHandle,
    pub pdu: Vec<u8>,
}

/// Hardware-independent interface for read-only SMS intake.
///
/// Each modem adapter owns its transport and AT commands. Consumers query
/// capabilities and health, list stored SMS handles, and read raw SMS-DELIVER
/// PDU bytes without knowing which modem or transport is used.
///
/// One owner calls an adapter serially. A handle identifies adapter storage,
/// not a durable message ID; adapters must reject stale handles even when a
/// modem reuses a storage slot. Decoding, persistence, and cloud upload belong
/// to separate layers.
///
/// These operations must not send or delete messages, or place/answer calls.
pub trait SimDeviceInterface {
    fn capabilities(&self) -> Capabilities;
    fn health(&mut self) -> Result<DeviceHealth, DeviceError>;
    fn list_pending_sms(&mut self) -> Result<Vec<SmsHandle>, DeviceError>;
    /// Invalid/stale handles must fail rather than returning a different slot's SMS.
    fn read_sms(&mut self, handle: &SmsHandle) -> Result<SmsSegment, DeviceError>;
}
