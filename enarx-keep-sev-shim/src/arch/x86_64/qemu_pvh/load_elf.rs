// SPDX-License-Identifier: Apache-2.0

use crate::memory::BootInfoFrameAllocator;

use crate::arch::x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
};

pub use x86_64::{PhysAddr, VirtAddr};

use crate::arch::x86_64::qemu_pvh::start_e820::{APP_SIZE, APP_START_PTR};
use x86_64::instructions::tlb::flush_all;
//FIXME: use x86_64::structures::paging::FrameDeallocator;
use crate::arch::x86_64::PHYSICAL_MEMORY_OFFSET;
use xmas_elf::program::{self, ProgramHeader64};

extern "C" {
    static mut PML2IDENT: [u64; 512];
}

pub(crate) fn load_elf(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> (*const u8, *const u8, usize) {
    use xmas_elf::program::ProgramHeader;

    // Extract required information from the ELF file.
    let entry_point;

    //#[cfg(debug_assertions)]
    unsafe {
        eprintln!("app start {:#X}", APP_START_PTR);
        eprintln!("app size {:#X}", APP_SIZE);
    }

    let app_bin = unsafe {
        core::slice::from_raw_parts(
            VirtAddr::new(APP_START_PTR + PHYSICAL_MEMORY_OFFSET).as_ptr(),
            APP_SIZE as _,
        )
    };

    let elf_file = xmas_elf::ElfFile::new(app_bin).unwrap();
    xmas_elf::header::sanity_check(&elf_file).unwrap();

    entry_point = elf_file.header.pt2.entry_point();

    let mut load_addr: Option<VirtAddr> = None;
    let mut elf_dyn_offset = 0_i64;

    for program_header in elf_file.program_iter() {
        match program_header {
            ProgramHeader::Ph64(header) => {
                let segment = *header;
                //println!("{:#?}", segment);
                if load_addr.is_none() && segment.get_type().unwrap() == program::Type::Load {
                    if segment.physical_addr == 0 {
                        // FIXME: find usable region instead and mark it used
                        elf_dyn_offset = 0x0040_0000;
                    }
                    load_addr.replace(
                        VirtAddr::new((segment.virtual_addr as i64 + elf_dyn_offset) as _)
                            - segment.offset,
                    );
                }
                map_user_segment(
                    &segment,
                    PhysAddr::new(unsafe { APP_START_PTR + PHYSICAL_MEMORY_OFFSET }),
                    elf_dyn_offset,
                    mapper,
                    frame_allocator,
                )
                .unwrap();
            }
            ProgramHeader::Ph32(_) => panic!("does not support 32 bit elf files"),
        }
    }
    eprintln!("entry_point {:#x}", entry_point);
    (
        (entry_point as i64 + elf_dyn_offset) as _,
        load_addr.unwrap().as_ptr(),
        elf_file.program_iter().count(),
    )
}

fn map_user_segment<T: FrameAllocator<Size4KiB> /*+ FrameDeallocator<Size4KiB>*/>(
    segment: &ProgramHeader64,
    file_start: PhysAddr,
    elf_dyn_offset: i64,
    page_table: &mut OffsetPageTable,
    frame_allocator: &mut T,
) -> Result<(), MapToError<Size4KiB>> {
    let typ = segment.get_type().unwrap();

    match typ {
        program::Type::Interp => {
            panic!("App is not a static binary");
        }
        program::Type::Load => {
            let mem_size = segment.mem_size;
            let file_size = segment.file_size;
            let file_offset = segment.offset;
            let phys_start_addr = file_start + file_offset;
            let virt_start_addr =
                VirtAddr::new(((segment.virtual_addr as i64) + elf_dyn_offset) as u64);

            unsafe {
                let src = core::slice::from_raw_parts(
                    phys_start_addr.as_u64() as *const u8,
                    file_size as _,
                );
                let dst = core::slice::from_raw_parts_mut(
                    virt_start_addr
                        .as_mut_ptr::<u8>()
                        .offset(PHYSICAL_MEMORY_OFFSET as isize),
                    file_size as _,
                );
                dst.copy_from_slice(src);

                core::ptr::write_bytes(
                    (virt_start_addr + file_size + PHYSICAL_MEMORY_OFFSET).as_mut_ptr::<u8>(),
                    0u8,
                    mem_size as usize - file_size as usize,
                );
            }
        }
        _ => {}
    }
    Ok(())
}
