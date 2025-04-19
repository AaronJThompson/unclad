# Fully Relocatable x86_64 Long Mode Trampoline
.intel_syntax noprefix

.org 0
.section .text
.global trampoline_start
.global trampoline_end
.global entry_point
.global stack_pointer
.global page_table_l4
.code16
trampoline_start:
    # Capture the current location using Intel syntax call
    mov eax, cs
    call get_rip

.align 8
entry_point: .8byte 0  # 64-bit entry point to jump to
stack_pointer: .8byte 0  # 64-bit stack pointer to use
page_table_l4: .4byte 0  # Physical address of PML4 table
jump_holder: .4byte 0  # Placeholder for jump address
    
.set ip_offset, get_rip - trampoline_start
get_rip:
    # EAX contains reset vector
    # EBX will be used to store the base address
    # Calculate base address

    xor ebx, ebx
    mov ebx, eax
    shl ebx, 4
    xor eax, eax

    mov [base_address], ebx      # Store base address for later use

    # Disable interrupts
    cli

    # A20 line enable
    in al, 0x92
    or al, 2
    out 0x92, al

    # Dynamically calculate GDT descriptor
    lea eax, [ebx + gdt_descriptor]
    mov [gdt_dynamic_address], eax

    # Load dynamically calculated GDT
    lgdt [gdt_dynamic_address]

    # Enable Protected Mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    # Relative far jump to protected mode

    jmp cs:protected_mode_entry
;     call compute_far_jump
    
; # Dynamic far jump computation
; compute_far_jump:
;     pop ax                      # Return address
;     lea eax, [protected_mode_entry]

.code32
protected_mode_entry:
    # Set up segments
    hlt
    hlt

    # BUG: Something is wrong here
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    hlt
    hlt
    # Enable PAE
    mov eax, cr4
    or eax, (1 << 5)  # PAE bit
    mov cr4, eax

    # BUG: Could have a problem here, jumps after paging could be incorrect
    mov eax, cr0
    or eax, 0x80000000   # Paging Enable
    mov cr0, eax

    hlt
    hlt

    # Prepare for long mode entry
    
    jmp cs:long_mode_entry

compute_long_mode_jump:
    pop eax                     # Return address
    push 0x08              # Code segment selector
    mov ebx, [base_address]
    lea eax, [ebx + long_mode_entry]
    push eax
    retf                        # Far return performs the jump

.code64
long_mode_entry:
    hlt
    hlt 
    hlt
    # Enable long mode MSR
    mov ecx, 0xC0000080  # EFER MSR
    rdmsr
    or eax, (1 << 8)     # Long Mode Enable
    wrmsr

    # Enable paging

    # Set up 64-bit segments
    xor rax, rax
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    # Load configuration
    mov rsp, [ebx + stack_pointer]

    # Maybe LEA entry point?
    lea rax, [ebx + entry_point]
    call rax

    # Halt if entry point returns

    cli
    hlt

.align 8
base_address: .2byte 0

# Dynamically computed GDT

gdt_start:
.quad 0x0000000000000000  # Null descriptor
.quad 0x00AF9A000000FFFF  # 64-bit Code Segment
.quad 0x00AF92000000FFFF  # 64-bit Data Segment
gdt_end:

# Dynamic GDT descriptor
gdt_dynamic_address: .4byte 0
gdt_descriptor:
    .word gdt_end - gdt_start - 1  # GDT size
    .quad 0                        # Placeholder for base (will be filled dynamically)

small_stack:
    .space 4096                   # 4KB stack space

trampoline_end: