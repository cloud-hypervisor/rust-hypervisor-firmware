// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2023 Rivos Inc.

use crate::mem::MemoryRegion;
use atomic_refcell::AtomicRefCell;
use chrono::{Datelike, NaiveDateTime, Timelike};

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

    let naive = NaiveDateTime::from_timestamp_opt(ts as i64, 0).ok_or(())?;
    let datetime = naive.and_utc();
    Ok((
        (datetime.year() - 2000) as u8,
        datetime.month() as u8,
        datetime.day() as u8,
    ))
}

pub fn read_time() -> Result<(u8, u8, u8), ()> {
    let ts = RTC_GOLDFISH.borrow_mut().read_ts();
    let naive = NaiveDateTime::from_timestamp_opt(ts as i64, 0).ok_or(())?;
    let datetime = naive.and_utc();
    Ok((
        datetime.hour() as u8,
        datetime.minute() as u8,
        datetime.second() as u8,
    ))
}
