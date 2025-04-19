use core::{arch::{asm, naked_asm}, hint, intrinsics::size_of_val, mem::transmute, ptr, sync::atomic::{AtomicU8, Ordering}};

use acpi::platform::ProcessorInfo;
use alloc::alloc::Global;
use x86::apic::{xapic::XAPIC, ApicControl, ApicId};
use x86_64::{structures::{gdt::{Descriptor, GlobalDescriptorTable}, paging::page}, PhysAddr, VirtAddr};

use crate::{memory::{active_level_4_table, phys_to_virt}, stack::STACK_REFS, x86_ext::FrameNumeric, PAGE_SIZE, PHYS_OFFSET};

static AP_GDT: GlobalDescriptorTable = {let mut gdt = GlobalDescriptorTable::new();
    gdt.append(Descriptor::kernel_code_segment());
    gdt.append(Descriptor::kernel_data_segment());
    gdt
};

const AP_BOOT_CODE: &[u8; include_bytes!(concat!(core::env!("OUT_DIR"), "/ap_boot.bin")).len()] = include_bytes!(concat!(core::env!("OUT_DIR"), "/ap_boot.bin"));

static AP_COUNTER: AtomicU8 = AtomicU8::new(0);

const BOOT_OFFSET_ENTRY: u64 = 0x08;
const BOOT_OFFSET_CPU_ID: u64 = BOOT_OFFSET_ENTRY + 0x08;
const BOOT_OFFSET_PML4: u64 = BOOT_OFFSET_CPU_ID + 0x04;
const BOOT_OFFSET_BASE_ADDR: u64 = BOOT_OFFSET_PML4 + 0x04;
const MMIO_REGION: u64 = 0xFEE00000;



#[unsafe(no_mangle)]
pub fn ap_main() -> ! {
    let cur_ap = AP_COUNTER.fetch_add(1, Ordering::SeqCst);
    log::info!("AP {} started", cur_ap);
    //TODO: Setup APIC, interrupt handlers, etc.
    loop {}
}

pub fn setup_cores(proc_info: ProcessorInfo<Global>) {
    log::debug!("Setting up cores");
    log::debug!("Switching to APIC mode on BSP");
    let mmio_region_addr = MMIO_REGION + unsafe {*PHYS_OFFSET.get_unchecked() as u64 };
    let mmio_region = unsafe { core::slice::from_raw_parts_mut(mmio_region_addr as *mut u32, 0x1000) };
    let mut bsp_apic = bsp_init_apic(mmio_region);
    log::debug!("BSP APIC initialized");
    log::debug!("Setting up AP trampoline");
    let trampoline_addr = PhysAddr::new(0x8000);
    let trampoline = copy_ap_trampoline(phys_to_virt(trampoline_addr));
    let tampoline_frame: FrameNumeric<PAGE_SIZE> = trampoline_addr.as_u64().try_into().unwrap();
    log::debug!("Trampoline copied to 0x8000");
    let pml4 = unsafe { active_level_4_table(VirtAddr::new(*PHYS_OFFSET.get().unwrap() as u64)) };
    log::debug!("Starting APs");
    for (i, cpu) in proc_info.application_processors.iter().enumerate() {
        log::debug!("CPU: {}, State: {:?}", i, cpu.state);
    }
    log::debug!("Startup vector {:x}", tampoline_frame.num);
    for (i, cpu) in proc_info.application_processors.iter().enumerate() {
        assign_trampoline_params(trampoline, i as u32, PhysAddr::new(pml4 as *const _ as u64));
        let apic_id = ApicId::XApic(cpu.local_apic_id as u8);
        unsafe { bsp_apic.ipi_init(apic_id) };
        for _ in 0..100000 {
            hint::spin_loop();
        }
        unsafe { bsp_apic.ipi_startup(apic_id, tampoline_frame.num as u8) };
        for _ in 0..100000 {
            hint::spin_loop();
        }
    }
    for (i, cpu) in proc_info.application_processors.iter().enumerate() {
        log::debug!("CPU: {}, State: {:?}", i, cpu.state);
    }
}

pub fn copy_ap_trampoline(target: VirtAddr) -> VirtAddr {
    log::debug!("AP trampoline size: {}", unsafe {size_of_val(AP_BOOT_CODE)});
    let aligned_target = target.align_down(4096 as u64);
    log::debug!("Copying AP trampoline to {:#x}", aligned_target);
    unsafe { ptr::copy_nonoverlapping(AP_BOOT_CODE,aligned_target.as_mut_ptr(), 1)};
    log::debug!("AP trampoline copied to {:#x}", aligned_target);
    let code: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(aligned_target.as_mut_ptr(), size_of_val(AP_BOOT_CODE)) };
    log::debug!("Assigning base addresses for trampoline");
    let ap_main_ptr = ap_main as *const () as u64;
    let ap_main_ref = &ap_main_ptr.to_le_bytes();
    log::debug!("AP main pointer: {:x}, IP: {:x}", ap_main_ptr, x86_64::registers::read_rip().as_u64());
    code[BOOT_OFFSET_ENTRY as usize..BOOT_OFFSET_ENTRY as usize + 8].copy_from_slice(ap_main_ref);
    code[BOOT_OFFSET_BASE_ADDR as usize..BOOT_OFFSET_BASE_ADDR as usize + 4].copy_from_slice(&(aligned_target.as_u64() as u32).to_le_bytes());
    aligned_target
}

pub fn assign_trampoline_params(trampoline: VirtAddr, cpu_id: u32, pml4: PhysAddr) {
    let code: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(trampoline.as_mut_ptr(), size_of_val(AP_BOOT_CODE)) };
    code[BOOT_OFFSET_CPU_ID as usize..BOOT_OFFSET_CPU_ID as usize + 4].copy_from_slice(&cpu_id.to_le_bytes());

    //BUG: Ensure PML4 is within 32-bit address space
    code[BOOT_OFFSET_PML4 as usize..BOOT_OFFSET_PML4 as usize + 4].copy_from_slice(&(pml4.as_u64() as u32).to_le_bytes());
}

pub fn bsp_init_apic(apic_region: &'static mut [u32]) -> XAPIC {
    let mut apic = XAPIC::new(apic_region);
    apic.attach();
    apic
}