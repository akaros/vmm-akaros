/* SPDX-License-Identifier: GPL-2.0-only */
#[inline]
pub fn round_up_4k(num: usize) -> usize {
    (num + 0xfff) & !0xfff
}

#[inline]
pub fn round_down_4k(num: usize) -> usize {
    num & !0xfff
}
