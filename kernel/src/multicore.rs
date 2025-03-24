use core::{arch::{asm, naked_asm}, ptr, sync::atomic::AtomicU8};

use acpi::platform::ProcessorInfo;
use alloc::alloc::Global;
use x86_64::{structures::gdt::{Descriptor, GlobalDescriptorTable}, VirtAddr};

static AP_GDT: GlobalDescriptorTable = {let mut gdt = GlobalDescriptorTable::new();
    gdt.append(Descriptor::kernel_code_segment());
    gdt.append(Descriptor::kernel_data_segment());
    gdt
};

static AP_COUNTER: AtomicU8 = AtomicU8::new(0);


//TODO: Check if the jumps to labels are absolute. They need to be relative to the current instruction pointer.
#[naked]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ap_bootstrap() {
    unsafe { naked_asm!(
        ".code16",
        // 1. Disable interrupts
        "cli",

        // 2. Setup a basic GDT (real mode doesn't have one by default)
        "lgdt {gdt_ptr}", // Load GDT
        
        // 3. Enable protected mode
        "mov eax, cr0",
        "or  eax, 1",
        "mov cr0, eax",
        
        // 4. Far jump to 32-bit protected mode
        "ljmp 2f",
        
        // 32-bit protected mode
        ".code32",
        "2:",
        "mov ax, 0x10", // Load data segment selector
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        
        // 5. Enable long mode (64-bit)
        "mov ecx, 0xC0000080",  // Read EFER MSR
        "rdmsr",
        "or eax, (1 << 8)",     // Set LME bit
        "wrmsr",
        
        // Enable PAE (Physical Address Extension)
        "mov eax, cr4",
        "or eax, (1 << 5)", // PAE
        "mov cr4, eax",
        
        // Enable paging
        "mov eax, cr0",
        "or eax, (1 << 31)", // Enable paging
        "mov cr0, eax",
        
        // 6. Jump to long mode
        "ljmp 3f",
        
        // 64-bit long mode
        ".code64",
        "3:",

        //TODO: DO THIS!!!!!!!!!!!!
        // "mov rsp, {stack}",   // Set up stack
        "call {entry}",       // Jump to Rust AP entry function
        entry = sym ap_main,
        gdt_ptr = sym AP_GDT,
    ) }
}

pub fn ap_main() {
    AP_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    loop {}
}

pub fn setup_cores(proc_info: ProcessorInfo<Global>) {
    
}

pub fn copy_ap_trampoline(target: VirtAddr) {
    unsafe { ptr::copy_nonoverlapping(&ap_bootstrap,target.align_down(4096 as u64).as_mut_ptr(), 1)};
}