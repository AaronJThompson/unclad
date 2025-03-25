use core::{arch::{asm, naked_asm}, intrinsics::size_of_val, ptr, sync::atomic::AtomicU8};

use acpi::platform::ProcessorInfo;
use alloc::alloc::Global;
use x86_64::{structures::gdt::{Descriptor, GlobalDescriptorTable}, PhysAddr, VirtAddr};

use crate::{stack::STACK_REFS};

static AP_GDT: GlobalDescriptorTable = {let mut gdt = GlobalDescriptorTable::new();
    gdt.append(Descriptor::kernel_code_segment());
    gdt.append(Descriptor::kernel_data_segment());
    gdt
};

static AP_COUNTER: AtomicU8 = AtomicU8::new(0);

const BOOT_OFFSET_ENTRY: u64 = 0x08;
const BOOT_OFFSET_CPU_ID: u64 = BOOT_OFFSET_ENTRY + 0x08;
const BOOT_OFFSET_PML4: u64 = BOOT_OFFSET_CPU_ID + 0x04;


#[unsafe(no_mangle)]
pub fn ap_main() -> ! {
    //We have no stack yet, assign rsp
    //ESI contains CPU ID
    unsafe {
        asm!("mov rsp, [{} + esi * 8]", in(reg) &STACK_REFS);
    }
    let cur_ap = AP_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    log::info!("AP {} started", cur_ap);
    //TODO: Setup APIC, interrupt handlers, etc.
    loop {}
}

pub fn setup_cores(proc_info: ProcessorInfo<Global>) {
    
}

pub fn copy_ap_trampoline(target: VirtAddr, page_table: PhysAddr) -> VirtAddr {
    let aligned_target = target.align_down(4096 as u64);
    let ap_boot_code = include_bytes!(concat!(core::env!("OUT_DIR"), "/ap_boot.bin"));
    unsafe { ptr::copy_nonoverlapping(&ap_bootstrap,aligned_target.as_mut_ptr(), 1)};
    let code: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(aligned_target.as_mut_ptr(), size_of_val(&ap_bootstrap)) };
    let page_addr = page_table.as_u64();
    let page_addr_bytes = page_addr.to_le_bytes();
    const PAGE_REPLACEMENT_PATTERN_BYTES: [u8; 8] = PAGE_REPLACEMENT_PATTERN.to_le_bytes();
    //Search code for page replacement pattern and replace it with the page table address
    for i in 0..code.len() {
        if code[i..i+8] == PAGE_REPLACEMENT_PATTERN_BYTES {
            code[i..i+8].copy_from_slice(&page_addr_bytes);
        }
    }
    aligned_target
}