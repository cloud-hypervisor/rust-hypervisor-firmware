// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2021 Akira Moroo

use atomic_refcell::AtomicRefCell;
use x86_64::instructions::port::{Port, PortWriteOnly};

static RTC: AtomicRefCell<Rtc> = AtomicRefCell::new(Rtc::new());

struct Rtc {
    address_port: PortWriteOnly<u8>,
    data_port: Port<u8>,
    reg_b: Option<u8>,
}

impl Rtc {
    const fn new() -> Self {
        Self {
            address_port: PortWriteOnly::new(0x70),
            data_port: Port::new(0x71),
            reg_b: None,
        }
    }

    fn read_cmos(&mut self, addr: u8) -> u8 {
        assert!(addr < 128);
        unsafe {
            self.address_port.write(addr);
            self.data_port.read()
        }
    }

    fn is_updating(&mut self) -> bool {
        self.read_cmos(0x0a) & 0x80 != 0
    }

    fn read(&mut self, offset: u8) -> Result<u8, ()> {
        if crate::delay::wait_while(1, || self.is_updating()) {
            return Err(());
        }
        Ok(self.read_cmos(offset))
    }

    fn get_reg_b(&mut self) -> u8 {
        if self.reg_b.is_none() {
            self.reg_b = Some(self.read_cmos(0x0b));
        }
        self.reg_b.unwrap()
    }

    fn read_date(&mut self) -> Result<(u8, u8, u8), ()> {
        let mut year = self.read(0x09)?;
        let mut month = self.read(0x08)?;
        let mut day = self.read(0x07)?;

        if (self.get_reg_b() & 0x04) == 0 {
            year = bcd2dec(year);
            month = bcd2dec(month);
            day = bcd2dec(day);
        }

        Ok((year, month, day))
    }

    fn read_time(&mut self) -> Result<(u8, u8, u8), ()> {
        let mut hour = self.read(0x04)?;
        let mut minute = self.read(0x02)?;
        let mut second = self.read(0x00)?;

        if (self.get_reg_b() & 0x04) == 0 {
            hour = bcd2dec(hour);
            minute = bcd2dec(minute);
            second = bcd2dec(second);
        }

        if ((self.get_reg_b() & 0x02) == 0) && ((hour & 0x80) != 0) {
            hour = (hour & 0x7f) + 12 % 24;
        }

        Ok((hour, minute, second))
    }
}

fn bcd2dec(b: u8) -> u8 {
    ((b >> 4) & 0x0f) * 10 + (b & 0x0f)
}

pub fn read_date() -> Result<(u8, u8, u8), ()> {
    RTC.borrow_mut().read_date()
}

pub fn read_time() -> Result<(u8, u8, u8), ()> {
    RTC.borrow_mut().read_time()
}
