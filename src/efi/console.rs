// SPDX-License-Identifier: Apache-2.0
// Copyright © 2019 Intel Corporation

use r_efi::{
    efi::{Boolean, Char16, Event, Handle, Status},
    protocols::{
        simple_text_input::{InputKey, Protocol as SimpleTextInputProtocol},
        simple_text_output::{Mode as SimpleTextOutputMode, Protocol as SimpleTextOutputProtocol},
    },
};

use super::{HandleType, HandleWrapper};

pub const STDIN_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;
pub const STDOUT_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;
pub const STDERR_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;

pub extern "efiapi" fn stdin_reset(_: *mut SimpleTextInputProtocol, _: Boolean) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn stdin_read_key_stroke(
    _: *mut SimpleTextInputProtocol,
    _: *mut InputKey,
) -> Status {
    Status::NOT_READY
}

pub extern "efiapi" fn stdout_reset(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    Status::SUCCESS
}

pub extern "efiapi" fn stdout_output_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {
    use core::fmt::Write;
    let mut serial = crate::serial::Serial;
    let mut string_end = false;

    loop {
        let mut output: [u8; 128] = [0; 128];
        let mut i: usize = 0;
        while i < output.len() {
            output[i] = (unsafe { *message.add(i) } & 0xffu16) as u8;
            if output[i] == 0 {
                string_end = true;
                break;
            }
            i += 1;
        }
        let s = unsafe { core::str::from_utf8_unchecked(&output) };
        serial.write_str(s).unwrap();
        if string_end {
            break;
        }
    }
    Status::SUCCESS
}

pub extern "efiapi" fn stdout_test_string(
    _: *mut SimpleTextOutputProtocol,
    _: *mut Char16,
) -> Status {
    Status::SUCCESS
}

pub extern "efiapi" fn stdout_query_mode(
    _: *mut SimpleTextOutputProtocol,
    mode: usize,
    columns: *mut usize,
    rows: *mut usize,
) -> Status {
    if mode == 0 {
        unsafe {
            *columns = 80;
            *rows = 25;
        }
        Status::SUCCESS
    } else {
        Status::UNSUPPORTED
    }
}

pub extern "efiapi" fn stdout_set_mode(_: *mut SimpleTextOutputProtocol, mode: usize) -> Status {
    if mode == 0 {
        Status::SUCCESS
    } else {
        Status::UNSUPPORTED
    }
}

pub extern "efiapi" fn stdout_set_attribute(_: *mut SimpleTextOutputProtocol, _: usize) -> Status {
    // Accept all attribute changes but ignore them
    Status::SUCCESS
}

pub extern "efiapi" fn stdout_clear_screen(_: *mut SimpleTextOutputProtocol) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn stdout_set_cursor_position(
    _: *mut SimpleTextOutputProtocol,
    _: usize,
    _: usize,
) -> Status {
    Status::UNSUPPORTED
}

pub extern "efiapi" fn stdout_enable_cursor(
    _: *mut SimpleTextOutputProtocol,
    _: Boolean,
) -> Status {
    Status::UNSUPPORTED
}

pub const STDIN: SimpleTextInputProtocol = SimpleTextInputProtocol {
    reset: stdin_reset,
    read_key_stroke: stdin_read_key_stroke,
    wait_for_key: 0 as Event,
};

pub const STDOUT_OUTPUT_MODE: SimpleTextOutputMode = SimpleTextOutputMode {
    max_mode: 1,
    mode: 0,
    attribute: 0,
    cursor_column: 0,
    cursor_row: 0,
    cursor_visible: Boolean::FALSE,
};

pub const STDOUT: SimpleTextOutputProtocol = SimpleTextOutputProtocol {
    reset: stdout_reset,
    output_string: stdout_output_string,
    test_string: stdout_test_string,
    query_mode: stdout_query_mode,
    set_mode: stdout_set_mode,
    set_attribute: stdout_set_attribute,
    clear_screen: stdout_clear_screen,
    set_cursor_position: stdout_set_cursor_position,
    enable_cursor: stdout_enable_cursor,
    mode: &STDOUT_OUTPUT_MODE as *const SimpleTextOutputMode as *mut SimpleTextOutputMode,
};
