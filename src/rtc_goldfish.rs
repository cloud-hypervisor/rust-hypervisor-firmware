// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Rivos Inc.

use crate::mem::MemoryRegion;
use atomic_refcell::AtomicRefCell;
use chrono::{DateTime, Datelike, Timelike};

// TODO: Fill from FDT
const RTC_GOLDFISH_ADDRESS: u64 = 0x101000;
static RTC_GOLDFISH: AtomicRefCell<RtcGoldfish> =
    AtomicRefCell::new(RtcGoldfish::new(RTC_GOLDFISH_ADDRESS));

pub struct RtcGoldfish {
    region: MemoryRegion,
}

impl RtcGoldfish {
    pub const fn new(base: u64) -> RtcGoldfish {
        RtcGoldfish {
            region: MemoryRegion::new(base, 8),
        }
    }

    fn read_ts(&self) -> u64 {
        const NSECS_PER_SEC: u64 = 1_000_000_000;

        let low = u64::from(self.region.io_read_u32(0x0));
        let high = u64::from(self.region.io_read_u32(0x04));

        let t = high << 32 | low;
        t / NSECS_PER_SEC
    }
}

pub fn read_date() -> Result<(u8, u8, u8), ()> {
    let ts = RTC_GOLDFISH.borrow_mut().read_ts();

    let datetime = DateTime::from_timestamp(ts as i64, 0).ok_or(())?;
    let date = datetime.date_naive();
    Ok((
        (date.year() - 2000) as u8,
        date.month() as u8,
        date.day() as u8,
    ))
}

pub fn read_time() -> Result<(u8, u8, u8), ()> {
    let ts = RTC_GOLDFISH.borrow_mut().read_ts();
    let datetime = DateTime::from_timestamp(ts as i64, 0).ok_or(())?;
    let time = datetime.time();
    Ok((time.hour() as u8, time.minute() as u8, time.second() as u8))
}
