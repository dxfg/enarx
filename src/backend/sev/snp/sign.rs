// SPDX-License-Identifier: Apache-2.0

use super::ByteSized;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Es384 {
    pub r: [u8; 0x48],
    pub s: [u8; 0x48],
}

impl Default for Es384 {
    fn default() -> Self {
        Self {
            r: [0u8; 0x48],
            s: [0u8; 0x48],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub component: Es384,
    rsvd: [u8; 368], // must be zero
}

impl Default for Signature {
    fn default() -> Self {
        Self {
            component: Es384::default(),
            rsvd: [0u8; 368],
        }
    }
}

impl ByteSized for Signature {}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PublicKey {
    pub curve: u32,
    pub component: Es384,
    rsvd: [u8; 880], // must be zero
}

impl Default for PublicKey {
    fn default() -> Self {
        Self {
            curve: 2,
            component: Es384::default(),
            rsvd: [0u8; 880],
        }
    }
}

impl ByteSized for PublicKey {}
