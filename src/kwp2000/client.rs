use super::Interface;

pub struct Client<I: Interface> {
    pub interface: I,
}

impl<I: Interface> Client<I> {}
