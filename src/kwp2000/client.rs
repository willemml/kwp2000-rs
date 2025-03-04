use crate::{
    Error,
    kwp2000::{
        constants::{ServiceError, ServiceId},
        response::ProcessError,
        security_key_from_seed,
    },
};

use super::{Interface, message::Message, response::Response};

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
    fn test(&mut self) -> Result<(), Error> {
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

    pub fn test2(&mut self) -> Result<(), Error> {
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
                    self.test()?;
                    return self.test2();
                }
            }
        }}
    }

    fn clear_security_wait(&mut self) -> Result<(), Error> {
        self.interface.send(Message::ClearLocalIdentifier(0xF0))?;

        match self.interface.next_response()? {
            Response::LocalIdentifierDefined(id) => self
                .interface
                .send(Message::DefineLocalIdentifierAddress(id, 2, 0x380da8))?,
            _ => return Err(Error::UnexpectedResponse),
        }

        match self.interface.next_response()? {
            Response::LocalIdentifierDefined(id) => {
                self.interface
                    .send(Message::WriteLocalIdentifier(id, vec![0, 0]))?;
                Ok(())
            }
            _ => Err(Error::UnexpectedResponse),
        }
    }

    pub fn get_security_access(&mut self) -> Result<(), Error> {
        self.interface.send(Message::RequestSecuritySeed)?;
        match self.interface.next_response()? {
            Response::SecurityAccessSeed(_, seed) => {
                self.interface
                    .send(Message::SendSecurityKey(u32::from_be_bytes(
                        seed.to_vec().try_into().unwrap(),
                    )))?
            }
            Response::SecurityAccessGranted(_) => return Ok(()),
            _ => return Err(Error::UnexpectedResponse),
        }

        match self.interface.next_response()? {
            Response::Error(ProcessError {
                error: ServiceError::TooManyAttempts | ServiceError::RequestingTooFast,
                service: ServiceId::SecurityAccess,
            }) => {
                self.clear_security_wait()?;
                self.get_security_access()
            }
            Response::SecurityAccessGranted(_) => Ok(()),
            _ => Err(Error::UnexpectedResponse),
        }
    }
}
