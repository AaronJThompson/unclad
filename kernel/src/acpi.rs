use core::ptr::NonNull;

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use x86_64::VirtAddr;

#[derive(Clone)]
struct OffsetMappedHandler {
    pub offset: VirtAddr,
}

impl AcpiHandler for OffsetMappedHandler {
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

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

pub fn parse_acpi(offset: VirtAddr, rsdp_addr: u64) {
    let tables = unsafe {
        AcpiTables::from_rsdp(OffsetMappedHandler { offset }, rsdp_addr as usize).unwrap()
    };
    let pt = tables.platform_info().unwrap();
    log::info!("{:?}", pt.processor_info.unwrap());
    log::info!("{:#?}", tables.platform_info());
}