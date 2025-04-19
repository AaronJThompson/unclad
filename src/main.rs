// src/main.rs

fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");
    
    // choose whether to start the UEFI or BIOS image
    let uefi = false;
    let debug = false;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    if debug {
        cmd.arg("-s");
        cmd.arg("-S");
    }
    cmd.arg("-no-reboot");
    cmd.arg("-no-shutdown");
    cmd.arg("-serial").arg("stdio");
    cmd.arg("-smp").arg("2");
    cmd.arg("-d").arg("guest_errors,cpu_reset,int");
    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive").arg(format!("format=raw,file={uefi_path}"));
        
    } else {

        cmd.arg("-drive").arg(format!("format=raw,file={bios_path}"));
    }
    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}