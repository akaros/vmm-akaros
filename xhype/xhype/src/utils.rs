/* SPDX-License-Identifier: GPL-2.0-only */
#[inline]
pub fn round_up(num: usize) -> usize {
    (num + 0xfff) & !0xfff
}

#[inline]
pub fn round_down(num: usize) -> usize {
    num & !0xfff
}
