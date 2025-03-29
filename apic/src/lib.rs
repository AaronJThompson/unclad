#![no_std]

use raw_cpuid::CpuId;

//XAPIC is APIC compatible, no need to differentiate
#[repr(u8)]
pub enum ApicMode {
    XApic,
    X2Apic,
}

pub fn get_apic_available() -> Option<ApicMode> {
    let feature_info = CpuId::new().get_feature_info()?;
    let x2apic = feature_info.has_x2apic();
    if !(feature_info.has_apic() | x2apic) {
        //No APIC available
        return None;
    }
    if x2apic {
        //Guys guys wait, what if we call it EXTENDED extended advanced PIC
        Some(ApicMode::X2Apic)
    } else {
        Some(ApicMode::XApic)
    }
}

