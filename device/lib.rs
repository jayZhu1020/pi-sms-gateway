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

/// One owner calls an adapter serially; tools never access raw transport.
/// These operations must not send, delete messages, or place/answer calls.
pub trait SimDeviceInterface {
    fn capabilities(&self) -> Capabilities;
    fn health(&mut self) -> Result<DeviceHealth, DeviceError>;
    fn list_pending_sms(&mut self) -> Result<Vec<SmsHandle>, DeviceError>;
    /// Invalid/stale handles must fail rather than returning a different slot's SMS.
    fn read_sms(&mut self, handle: &SmsHandle) -> Result<SmsSegment, DeviceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeDevice {
        connected: bool,
        supported: bool,
        messages: Vec<SmsSegment>,
    }

    impl SimDeviceInterface for FakeDevice {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                receive_sms: self.supported,
            }
        }
        fn health(&mut self) -> Result<DeviceHealth, DeviceError> {
            if !self.connected {
                return Err(DeviceError::Disconnected);
            }
            Ok(DeviceHealth {
                sim_ready: true,
                registered: true,
            })
        }
        fn list_pending_sms(&mut self) -> Result<Vec<SmsHandle>, DeviceError> {
            self.health()?;
            if !self.supported {
                return Err(DeviceError::Unsupported);
            }
            Ok(self.messages.iter().map(|m| m.handle.clone()).collect())
        }
        fn read_sms(&mut self, handle: &SmsHandle) -> Result<SmsSegment, DeviceError> {
            self.list_pending_sms()?;
            self.messages
                .iter()
                .find(|m| &m.handle == handle)
                .cloned()
                .ok_or(DeviceError::InvalidHandle)
        }
    }

    fn fake() -> FakeDevice {
        FakeDevice {
            connected: true,
            supported: true,
            messages: vec![SmsSegment {
                handle: SmsHandle("fixture-receipt-1".into()),
                pdu: vec![0x01, 0x02],
            }],
        }
    }

    #[test]
    fn consumer_reads_via_trait_without_mutating_storage() {
        let mut adapter = fake();
        let device: &mut dyn SimDeviceInterface = &mut adapter;
        assert!(device.capabilities().receive_sms);
        assert!(device.health().unwrap().sim_ready);
        let handles = device.list_pending_sms().unwrap();
        assert_eq!(device.read_sms(&handles[0]).unwrap().pdu, vec![0x01, 0x02]);
        assert_eq!(device.list_pending_sms().unwrap(), handles);
    }

    #[test]
    fn unsupported_and_disconnected_are_distinct() {
        let mut device = fake();
        device.supported = false;
        assert!(!device.capabilities().receive_sms);
        assert_eq!(device.list_pending_sms(), Err(DeviceError::Unsupported));
        device.connected = false;
        assert_eq!(device.health(), Err(DeviceError::Disconnected));
        assert_eq!(
            device.read_sms(&SmsHandle("missing".into())),
            Err(DeviceError::Disconnected)
        );
    }

    #[test]
    fn stale_handle_cannot_read_another_message() {
        let mut device = fake();
        assert_eq!(
            device.read_sms(&SmsHandle("old-receipt".into())),
            Err(DeviceError::InvalidHandle)
        );
    }
}
