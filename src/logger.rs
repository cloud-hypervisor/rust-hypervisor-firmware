// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2024 Akira Moroo

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            log!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_logger(&Logger).expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Info);
}
