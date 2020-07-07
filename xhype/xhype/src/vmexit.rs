/* SPDX-License-Identifier: GPL-2.0-only */

use crate::err::Error;
use crate::hv::vmx::*;
use crate::{GuestThread, VCPU};
#[allow(unused_imports)]
use log::*;

#[derive(Debug, Eq, PartialEq)]
pub enum HandleResult {
    Exit,
    Resume,
    Next,
}

////////////////////////////////////////////////////////////////////////////////
// VMX_REASON_EPT_VIOLATION
////////////////////////////////////////////////////////////////////////////////

pub fn ept_read(qual: u64) -> bool {
    qual & 1 > 0
}

pub fn ept_write(qual: u64) -> bool {
    qual & 0b10 > 0
}

pub fn ept_instr_fetch(qual: u64) -> bool {
    qual & 0b100 > 0
}

pub fn ept_page_walk(qual: u64) -> bool {
    qual & (1 << 7) > 0 && qual & (1 << 8) == 0
}

pub fn handle_ept_violation(
    _vcpu: &VCPU,
    _gth: &mut GuestThread,
    _gpa: usize,
) -> Result<HandleResult, Error> {
    // we need to handle MMIOs. But for now we just resume the vm.
    Ok(HandleResult::Resume)
}
