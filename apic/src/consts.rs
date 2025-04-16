
//These const names are by specification, however IA32 does not imply narrow 32-bit compatality
//APIC is fairly similar between architectures

const IA32_APIC_BASE_MSR: usize = 0x1B;
const IA32_APIC_BASE_MSR_BSP: usize = 1 << 8;
const IA32_APIC_BASE_MSR_ENABLE: usize = 1 << 11;
const IA32_APIC_BASE_MSR_X2APIC_ENABLE: usize = 1 << 10;
const IA32_APIC_BASE_MSR_BASE_ADDR: usize = 1 << 12;