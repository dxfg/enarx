// SPDX-License-Identifier: Apache-2.0

use super::syscall;
use crate::arch::x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
};
use crate::arch::x86_64::{
    NEXT_MMAP, PHYSICAL_MEMORY_OFFSET, USER_APP_OFFSET, USER_STACK_OFFSET, USER_STACK_SIZE,
};
use crate::memory::BootInfoFrameAllocator;
use crate::{exit_hypervisor, HyperVisorExitCode};
use crt0stack::{self, Builder, Entry};
use x86_64::instructions::random::RdRand;
use x86_64::structures::paging::{PhysFrame, UnusedPhysFrame};
use x86_64::{PhysAddr, VirtAddr};

pub fn exec_elf(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootInfoFrameAllocator,
    _app_entry_point: *const u8,
    app_load_addr: *const u8,
    _app_phnum: usize,
) -> ! {
    let rdrand = RdRand::new();
    let (r1, r2, r3) = match rdrand {
        None => {
            if cfg!(debug_assertions) {
                eprintln!("!!! No RDRAND. Using pseudo random numbers!!!");
                (
                    0xAFFE_AFFE_AFFE_AFFE_u64,
                    0xC0FF_EEC0_FFEE_C0FF_u64,
                    0xFCFC_0000_u64,
                )
            } else {
                panic!("No rdrand supported by CPU")
            }
        }
        Some(rdrand) => (
            rdrand.get_u64().unwrap(),
            rdrand.get_u64().unwrap(),
            rdrand.get_u64().unwrap(),
        ),
    };

    let stack_start_addr = VirtAddr::new(USER_STACK_OFFSET as u64 + ((r3 & 0xFFFFF) << 12));
    let start_page: Page = Page::containing_address(stack_start_addr);
    let end_page: Page = Page::containing_address(stack_start_addr + USER_STACK_SIZE - 256u64);
    let page_range = Page::range_inclusive(start_page, end_page);
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::<Size4KiB>::FrameAllocationFailed)
            .unwrap();
        mapper
            .map_to(
                page,
                frame,
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE,
                PageTableFlags::USER_ACCESSIBLE,
                frame_allocator,
            )
            .unwrap()
            .flush();
    }

    {
        eprintln!("\n========= ASLR =============\n");
        eprintln!("app_entry_point={:#X}", _app_entry_point as u64);
        eprintln!("app_load_addr={:#X}", app_load_addr as u64);
        eprintln!("app_phnum={}", _app_phnum);
        eprintln!("\n========= ASLR =============\n");
    }

    let app_load_addr = if (app_load_addr as usize as u64) > PHYSICAL_MEMORY_OFFSET {
        unsafe { app_load_addr.offset(-1 * PHYSICAL_MEMORY_OFFSET as isize) }
    } else {
        app_load_addr
    };

    use goblin::elf::header::header64::Header;
    let elf_header =
        unsafe { app_load_addr.offset(PHYSICAL_MEMORY_OFFSET as isize) as *const Header };
    unsafe {
        eprintln!("(*elf_header).e_entry = {:#X}", (*elf_header).e_entry);
        eprintln!("(*elf_header).e_phnum = {}", (*elf_header).e_phnum);
    }
    let app_entry_point =
        unsafe { app_load_addr.offset((*elf_header).e_entry as isize) } as usize as *const u8;
    let app_phnum: usize = unsafe { (*elf_header).e_phnum } as _;

    use goblin::elf::program_header::program_header64::ProgramHeader;
    let headers: &[ProgramHeader] = unsafe {
        core::slice::from_raw_parts(
            (PHYSICAL_MEMORY_OFFSET + app_load_addr as u64 + ELF64_HDR_SIZE)
                as *const ProgramHeader,
            app_phnum,
        )
    };

    let random_offset: i64 = ((r3 & 0xFFFFF0_0000) >> 8) as i64 + USER_APP_OFFSET as i64;
    unsafe {
        NEXT_MMAP += (r3 & 0xFFF_FF00_0000_0000) >> 20;
        eprintln!("NEXT_MMAP: {:#X}", NEXT_MMAP);
    }

    for header in headers {
        if header.p_type != goblin::elf::program_header::PT_LOAD {
            continue;
        }
        let start = header.p_vaddr + app_load_addr as u64;
        let end = start + header.p_memsz - 1;
        //println!("{:#X} - {:#X}", start, end);

        let start_page: Page = Page::containing_address(VirtAddr::new(start));
        let end_page: Page = Page::containing_address(VirtAddr::new(end));
        let page_range = Page::range_inclusive(start_page, end_page);
        //println!("{:#?}", page_range);

        let mut page_table_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

        if (header.p_flags & goblin::elf::program_header::PF_W) != 0 {
            page_table_flags |= PageTableFlags::WRITABLE
        }
        if (header.p_flags & goblin::elf::program_header::PF_X) == 0 {
            page_table_flags |= PageTableFlags::NO_EXECUTE
        }

        for page in page_range {
            //eprintln!("{:#?}", page);
            let frame = unsafe {
                UnusedPhysFrame::new(PhysFrame::containing_address(PhysAddr::new(
                    page.start_address().as_u64(),
                )))
            };
            let page = page + (random_offset as u64) / 4096;
            //eprintln!("{:#?}, {:#?}", page, page_table_flags);
            mapper
                .map_to(
                    page,
                    frame,
                    page_table_flags,
                    PageTableFlags::USER_ACCESSIBLE,
                    frame_allocator,
                )
                .and_then(|f| {
                    f.flush();
                    Ok(())
                })
                .or_else(|e| match e {
                    MapToError::PageAlreadyMapped(_) => Ok(()),
                    _ => Err(e),
                })
                .unwrap();
        }
    }

    let app_load_addr = unsafe { app_load_addr.offset(random_offset as isize) };
    let app_entry_point = unsafe { app_entry_point.offset(random_offset as isize) };

    const ELF64_HDR_SIZE: u64 = goblin::elf::header::header64::SIZEOF_EHDR as u64;
    const ELF64_PHDR_SIZE: u64 = goblin::elf::program_header::program_header64::SIZEOF_PHDR as u64;

    let hwcap = unsafe { core::arch::x86_64::__cpuid(1) }.edx;

    let mut ra = [0u8; 16];
    let r1u8 = unsafe { core::slice::from_raw_parts(&r1 as *const u64 as *const u8, 8) };
    let r2u8 = unsafe { core::slice::from_raw_parts(&r2 as *const u64 as *const u8, 8) };
    ra[0..8].copy_from_slice(r1u8);
    ra[8..16].copy_from_slice(r2u8);

    let mut sp_slice = unsafe {
        core::slice::from_raw_parts_mut(stack_start_addr.as_ptr::<u8>() as *mut u8, USER_STACK_SIZE)
    };

    let mut builder = Builder::new(&mut sp_slice);
    builder.push("/init").unwrap();
    builder.push("arg1").unwrap();
    builder.push("arg2").unwrap();
    let mut builder = builder.done().unwrap();
    builder.push("LANG=C").unwrap();
    let mut builder = builder.done().unwrap();
    for aux in &[
        Entry::ExecFilename("/init"),
        Entry::Platform("x86_64"),
        Entry::Uid(1000),
        Entry::EUid(1000),
        Entry::Gid(1000),
        Entry::EGid(1000),
        Entry::PageSize(4096),
        Entry::Secure(false),
        Entry::ClockTick(100),
        Entry::Flags(0),
        // FIXME: maybe later: Entry::Base((app_load_addr as u64 + 0x0) as _),
        Entry::PHdr((app_load_addr as u64 + ELF64_HDR_SIZE) as _),
        Entry::PHent(ELF64_PHDR_SIZE as _),
        Entry::PHnum(app_phnum),
        Entry::HwCap(hwcap as _),
        Entry::HwCap2(0),
        Entry::Random(ra),
    ] {
        builder.push(aux).unwrap();
    }
    let handle = builder.done().unwrap();
    let sp = handle.start_ptr() as *const () as usize;

    //#[cfg(debug_assertions)]
    {
        eprintln!("app_entry_point={:#X}", app_entry_point as u64);
        eprintln!("app_load_addr={:#X}", app_load_addr as u64);
        eprintln!("app_phnum={}", app_phnum);
        eprintln!("stackpointer={:#X}", sp);
        eprintln!("USER_STACK_OFFSET={:#X}", USER_STACK_OFFSET);
        eprintln!("\n========= APP START =============\n");
    }

    if app_entry_point.is_null() {
        eprintln!("app_entry_point.is_null()");
        exit_hypervisor(HyperVisorExitCode::Success);
        crate::hlt_loop()
    } else {
        unsafe {
            syscall::usermode(app_entry_point as usize, sp, 0);
        }
    }
}
