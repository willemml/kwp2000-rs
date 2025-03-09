use crate::{
    Error,
    kwp2000::{
        constants::{ServiceError, ServiceId},
        response::ProcessError,
        security_key_from_seed,
    },
};

use super::{Interface, constants::DiagnosticMode, message::Message, response::Response};

pub struct Client<I: Interface> {
    pub interface: I,
}

macro_rules! message_chain {
    {$interface:expr => {
        $($message:expr => {
            $($response:pat => $respond:block)*
        })*
    }} => {
        $(
            $interface.send($message)?;

            match $interface.next_response()? {
                $($response => $respond,)*
                r => {dbg!(r); return Err(Error::UnexpectedResponse);},
            }
        )*
    };
}

impl<I: Interface> Client<I> {
    pub fn disconnect(&mut self) -> Result<(), Error> {
        message_chain! {self.interface => {
            Message::StopDiagnosticSession => {
                Response::DiagnosticSessionStopped => {}
                Response::Error(_) => {}
            }
            Message::StopCommunication => {
                Response::CommunicationStopped => {}
            }
        }}

        Ok(())
    }

    pub fn switch_mode(
        &mut self,
        new_mode: DiagnosticMode,
        baud_rate: Option<u32>,
    ) -> Result<(), Error> {
        message_chain! {self.interface => {
            Message::StartDiagnosticSession(new_mode, baud_rate) => {
                Response::StartedDiagnosticMode(mode, new_baud) => {
                    if let Some(baud) = new_baud {
                        self.interface.switch_baud(baud)?;
                    }
                    if mode == new_mode {
                        Ok(())
                    } else {
                        Err(Error::UnexpectedMode)
                    }
                }
            }
        }}
    }
    pub fn clear_security_wait(&mut self) -> Result<(), Error> {
        message_chain! {self.interface => {
            Message::ClearLocalIdentifier(0xF0) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::DefineLocalIdentifierAddress(0xF0, 2, 0x380da8) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::WriteLocalIdentifier(0xF0, vec![0,0]) => {
                Response::LocalIdentifierWritten(0xF0) => {return Ok(())}
            }
        }}
    }

    pub fn get_security_access(&mut self) -> Result<(), Error> {
        let seed_arr;
        message_chain! {self.interface => {
            Message::RequestSecuritySeed => {
                Response::SecurityAccessSeed(_, seed) => {
                    seed_arr = seed.to_vec().try_into().unwrap();
                }
                Response::SecurityAccessGranted(_) => {
                    return Ok(());
                }
            }
            Message::SendSecurityKey(security_key_from_seed(seed_arr)) => {
                Response::SecurityAccessGranted(_) => {
                    return Ok(());
                }
                Response::Error(ProcessError {
                    error: ServiceError::TooManyAttempts | ServiceError::RequestingTooFast,
                    service: ServiceId::SecurityAccess,
                }) => {
                    self.clear_security_wait()?;
                    return self.get_security_access();
                }
            }
        }}
    }
}
