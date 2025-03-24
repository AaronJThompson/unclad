#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(static_mut_refs)]
#![feature(allocator_api)]
#![feature(naked_functions)]

extern crate alloc;
extern crate bootloader_api;

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use bootloader_api::{config::Mapping, info::FrameBufferInfo};
use bootloader_x86_64_common::logger::LockedLogger;
use buddy_system_allocator::{LockedFrameAllocator, LockedHeap};
use conquer_once::spin::OnceCell;
use memory::{allocate_heap, assign_frames};
use multicore::copy_ap_trampoline;
use core::{cell::UnsafeCell, panic::PanicInfo, ptr::NonNull};
use x86_64::{
    instructions::{interrupts, port::Port}, registers::{
        control::{Cr0Flags, Cr4Flags},
        segmentation::{Segment, CS},
    }, set_general_handler, structures::{
        idt::{InterruptDescriptorTable, InterruptStackFrame},
        paging::{PageSize, Size2MiB, Size4KiB},
    }, PrivilegeLevel, VirtAddr
};

mod memory;
mod x86_ext;
mod multicore;
mod stack;

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::empty();

pub(crate) const MAX_PROC_COUNT: usize = 32;
pub(crate) const MAX_STACK_SIZE: usize = 0x8000;

static mut FRAME_ALLOC: OnceCell<LockedFrameAllocator<32>> = OnceCell::uninit();

// ...
pub(crate) static LOGGER: OnceCell<LockedLogger> = OnceCell::uninit();
pub(crate) static PHYS_OFFSET: OnceCell<usize> = OnceCell::uninit();
pub(crate) static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();
pub(crate) type PAGE_SIZE = Size4KiB;
pub(crate) fn init_logger(buffer: &'static mut [u8], info: FrameBufferInfo) {
    let logger = LOGGER.get_or_init(move || LockedLogger::new(buffer, info, false, true));
    log::set_logger(logger).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}
pub(crate) fn init_frame_alloc() {
    unsafe { FRAME_ALLOC.init_once(|| LockedFrameAllocator::new()) };
}

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024; // 100 KiB
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

bootloader_api::entry_point!(kernel_main, config = &CONFIG);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    copy_ap_trampoline(VirtAddr::new(0x1000));
    let physical_offset = boot_info.physical_memory_offset.into_option().unwrap();
    PHYS_OFFSET.init_once(|| physical_offset as usize);
    let frame_buffer = boot_info.framebuffer.as_mut().unwrap();
    let frame_buffer_info = frame_buffer.info().clone();
    let raw_frame_buffer = frame_buffer.buffer_mut();
    for byte in &mut *raw_frame_buffer {
        *byte = 0x00;
    }
    init_logger(raw_frame_buffer, frame_buffer_info);
    log::info!("Logger initialized");
    init_frame_alloc();
    log::info!("Frame allocator initialized");
    let mut frame_alloc = unsafe { FRAME_ALLOC.get().unwrap().lock() };
    log::trace!("Frame allocator locked");
    assign_frames::<PAGE_SIZE, 32>(&boot_info.memory_regions, &mut frame_alloc);
    log::debug!("Frames assigned");
    allocate_heap::<PAGE_SIZE, 32>(&mut frame_alloc);
    log::info!("Heap allocated");
    log_cpu_mode();
    unsafe { IDT.load() };
    unsafe { set_general_handler!(&mut IDT, my_general_handler) };
    let mut physical_map = unsafe {
        memory::get_active_opt(VirtAddr::new(
            boot_info.physical_memory_offset.into_option().unwrap(),
        ))
    };
    assert_cpu_state(
        PrivilegeLevel::Ring0,
        Cr4Flags::PHYSICAL_ADDRESS_EXTENSION,
        Cr0Flags::PROTECTED_MODE_ENABLE | Cr0Flags::PAGING,
    );
    parse_acpi(
        physical_map.phys_offset(),
        boot_info.rsdp_addr.into_option().unwrap(),
    );
    loop {}
}

#[derive(Clone)]
struct OffsetMappedHandler {
    pub offset: VirtAddr,
}

impl AcpiHandler for OffsetMappedHandler {
    // TODO FIXME: This inline(never) annotation is required. Without it,
    // LLVM replaces the `search_for_on_bios` call below with a `ud2`
    // instruction. See https://github.com/rust-osdev/bootloader/issues/425
    #[inline(never)]
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new((self.offset + physical_address as u64).as_mut_ptr()).unwrap(),
                size,
                size,
                self.clone(),
            )
        }
    }

    fn unmap_physical_region<T>(_: &PhysicalMapping<Self, T>) {}
}

fn parse_acpi(offset: VirtAddr, rsdp_addr: u64) {
    let tables = unsafe {
        AcpiTables::from_rsdp(OffsetMappedHandler { offset }, rsdp_addr as usize).unwrap()
    };
    let pt = tables.platform_info().unwrap();
    log::info!("{:?}", pt.processor_info.unwrap());
    log::info!("{:#?}", tables.platform_info());
}

fn my_general_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    log::info!(
        "Interrupt: {}, ErrorCode: {}, PL: {:?}",
        index,
        error_code.unwrap_or(0),
        stack_frame.code_segment.rpl()
    );
}

fn setup_periodic_interrupt(freq: u32) {
    let divisor = 1193180 / freq;
    let mut pit_mode = Port::new(0x43);
    let mut pit_channel0 = Port::new(0x40);
    unsafe {
        interrupts::disable();
        pit_mode.write(0x36 as u8);
        pit_channel0.write((divisor & 0xFF) as u8);
        pit_channel0.write(((divisor >> 8) & 0xFF) as u8);
    }
}

fn log_cpu_mode() {
    let cr0 = x86_64::registers::control::Cr0::read();
    let cr4 = x86_64::registers::control::Cr4::read();
    let protected = cr0.contains(Cr0Flags::PROTECTED_MODE_ENABLE);
    let paging = cr0.contains(Cr0Flags::PAGING);
    let long_mode = cr4.contains(Cr4Flags::PHYSICAL_ADDRESS_EXTENSION);
    let cs = CS::get_reg();
    let ring_level = cs.rpl();
    log::trace!(
        "Protected: {}; Paging: {}, Long mode: {}, Ring Level: {:?}",
        protected,
        paging,
        long_mode,
        ring_level
    );
}

fn assert_cpu_state(privilege_level: PrivilegeLevel, cr4_flags: Cr4Flags, cr0_flags: Cr0Flags) {
    let cs = CS::get_reg();
    let ring_level = cs.rpl();
    assert_eq!(ring_level, privilege_level, "Ring level mismatch");
    let cr4 = x86_64::registers::control::Cr4::read();
    assert!(cr4.contains(cr4_flags), "CR4 flag mismatch");
    let cr0 = x86_64::registers::control::Cr0::read();
    assert!(cr0.contains(cr0_flags), "CR0 flag mismatch");
}

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
