// Copyright © 2019 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use r_efi::efi::{Boolean, Char16, Event, Handle, Status};
use r_efi::protocols::simple_text_input::InputKey;
use r_efi::protocols::simple_text_input::Protocol as SimpleTextInputProtocol;
use r_efi::protocols::simple_text_output::Mode as SimpleTextOutputMode;
use r_efi::protocols::simple_text_output::Protocol as SimpleTextOutputProtocol;

#[cfg(not(test))]
use super::{HandleType, HandleWrapper};

#[cfg(not(test))]
pub const STDIN_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;
#[cfg(not(test))]
pub const STDOUT_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;
#[cfg(not(test))]
pub const STDERR_HANDLE: Handle = &HandleWrapper {
    handle_type: HandleType::None,
} as *const _ as Handle;

#[cfg(not(test))]
pub extern "win64" fn stdin_reset(_: *mut SimpleTextInputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdin_reset\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdin_read_key_stroke(
    _: *mut SimpleTextInputProtocol,
    _: *mut InputKey,
) -> Status {
    Status::NOT_READY
}

#[cfg(not(test))]
pub extern "win64" fn stdout_reset(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_reset\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_output_string(
    _: *mut SimpleTextOutputProtocol,
    message: *mut Char16,
) -> Status {
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
        crate::log!("{}", unsafe { core::str::from_utf8_unchecked(&output) });
        if string_end {
            break;
        }
    }
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_test_string(
    _: *mut SimpleTextOutputProtocol,
    _: *mut Char16,
) -> Status {
    Status::SUCCESS
}

#[cfg(not(test))]
pub extern "win64" fn stdout_query_mode(
    _: *mut SimpleTextOutputProtocol,
    _: usize,
    _: *mut usize,
    _: *mut usize,
) -> Status {
    crate::log!("EFI_STUB: stdout_query_mode\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_mode(_: *mut SimpleTextOutputProtocol, _: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_mode\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_attribute(_: *mut SimpleTextOutputProtocol, _: usize) -> Status {
    crate::log!("EFI_STUB: stdout_set_attribute\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_clear_screen(_: *mut SimpleTextOutputProtocol) -> Status {
    crate::log!("EFI_STUB: stdout_clear_screen\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_set_cursor_position(
    _: *mut SimpleTextOutputProtocol,
    _: usize,
    _: usize,
) -> Status {
    crate::log!("EFI_STUB: stdout_set_cursor_position\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub extern "win64" fn stdout_enable_cursor(_: *mut SimpleTextOutputProtocol, _: Boolean) -> Status {
    crate::log!("EFI_STUB: stdout_enable_cursor\n");
    Status::UNSUPPORTED
}

#[cfg(not(test))]
pub const STDIN: SimpleTextInputProtocol = SimpleTextInputProtocol {
    reset: stdin_reset,
    read_key_stroke: stdin_read_key_stroke,
    wait_for_key: 0 as Event,
};

#[cfg(not(test))]
pub const STDOUT_OUTPUT_MODE: SimpleTextOutputMode = SimpleTextOutputMode {
    max_mode: 1,
    mode: 0,
    attribute: 0,
    cursor_column: 0,
    cursor_row: 0,
    cursor_visible: Boolean::FALSE,
};

#[cfg(not(test))]
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
