use std::time::Duration;

use crate::kwp2000::message::RawMessage;

pub trait KLine {
    type Error;

    fn send_init_5baud(&mut self, address: u8) -> Result<(), Self::Error> {
        // Idle for 300ms before sending anything.
        self.set_low()?;
        self.delay(Duration::from_millis(300));

        // Send high bit to start transfer.
        self.set_high()?;
        self.delay(Duration::from_millis(200));

        // Send target address at 5 baud
        self.bitbang(5, address)?;
        Ok(())
    }

    fn init_kwp2000(&mut self, address: u8) -> Result<(), Self::Error> {
        self.send_init_5baud(address)?;

        self.wait_for_byte(0x55)?;

        self.wait_for_byte(0x8F)?;

        // Wait a bit before sending complement of key byte 2
        self.delay(Duration::from_millis(25));
        self.write_byte(0xFF - 0x8F)?;

        self.wait_for_byte(0xFF - address)?;

        Ok(())
    }

    fn bitbang(&mut self, baud: u8, byte: u8) -> Result<(), Self::Error> {
        let delay = Duration::from_millis(1_000 / baud as u64);

        for state in (0..8).map(|n| ((1 << n) & byte) == 0) {
            if state {
                self.set_high()?;
            } else {
                self.set_low()?;
            }
            self.delay(delay);
        }

        // Set low to allow incoming data
        self.set_low()?;

        Ok(())
    }

    fn wait_for_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        while self.read_byte()? != byte {
            continue;
        }
        Ok(())
    }

    fn delay(&self, duration: Duration);

    fn write_message(&mut self, message: RawMessage) -> Result<(), Self::Error>;

    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;
    fn read_byte(&mut self) -> Result<u8, Self::Error>;

    fn set_high(&mut self) -> Result<(), Self::Error>;
    fn set_low(&mut self) -> Result<(), Self::Error>;
}

#[cfg(feature = "serialport")]
impl KLine for Box<dyn serialport::SerialPort> {
    type Error = serialport::Error;

    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn write_message(&mut self, message: RawMessage) -> Result<(), Self::Error> {
        self.write_all(&message.to_bytes())?;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_break()
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.clear_break()
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        self.write_all(&[byte])?;
        Ok(())
    }

    fn delay(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
