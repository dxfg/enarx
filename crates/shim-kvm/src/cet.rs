// SPDX-License-Identifier: Apache-2.0

//! CET related functions

use crate::snp::cpuid_count;

use x86_64::registers::control::{Cr4, Cr4Flags};
use x86_64::registers::model_specific::{SCet, CetFlags};

/// Setup and check CET compatability and execute relevant stuff
#[cfg_attr(coverage, no_coverage)]
pub fn init_cet() {
    const SHADOWSTACK_SUPPORTED_BIT: u32 = 1 << 7;
    let shadowstack_supported = (cpuid_count(7, 0).edx & SHADOWSTACK_SUPPORTED_BIT) != 0;
    assert!(shadowstack_supported);

    const IBT_SUPPORTED_BIT: u32 = 1 << 20;
    let ibt_supported = (cpuid_count(7, 1).edx & IBT_SUPPORTED_BIT) != 0;
    assert!(ibt_supported);

    let cet_supported = shadowstack_supported && ibt_supported;

    if cet_supported {
        let mut cr4 = Cr4::read();
        cr4 |= Cr4Flags::CONTROL_FLOW_ENFORCEMENT;
        unsafe { Cr4::write(cr4) };

	let (mut cur_flags, page_addr) = SCet::read();
	cur_flags |= CetFlags::SS_ENABLE | CetFlags::IBT_ENABLE;
        unsafe { SCet::write(cur_flags, page_addr) };
    }
}
