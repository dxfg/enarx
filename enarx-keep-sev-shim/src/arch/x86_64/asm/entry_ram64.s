# SPDX-License-Identifier: Apache-2.0

.section .entry64, "ax"
.global _start
.global _setup_pto
.code64

.p2align 4
_start:
    movabs $_start_main,%rax

# %rax  = jmp to start function
# %rdi  = first parameter for start function
.p2align 4
_setup_pto:
    mov    %rdi, %r11
    mov    %rax, %r12

    mov    %cr4,%rax
    or     $0x50620,%rax # FSGSBASE | PAE | OSFXSR | OSXMMEXCPT | OSXSAVE
    mov    %rax,%cr4

    mov    %cr0,%rax
    and    $0x60050009,%eax # mask EMULATE_COPROCESSOR | MONITOR_COPROCESSOR
    mov    $0x80000021,%ecx # | PROTECTED_MODE_ENABLE | NUMERIC_ERROR | PAGING
    or     %rax,%rcx
    mov    %rcx,%cr0

    # EFER |= LONG_MODE_ACTIVE | LONG_MODE_ENABLE | NO_EXECUTE_ENABLE | SYSTEM_CALL_EXTENSIONS
    mov    $0xc0000080,%ecx
    rdmsr
    or     $0xd01,%eax
    mov    $0xc0000080,%ecx
    wrmsr

    mov  $PML4T, %eax
    mov  %rax, %cr3
    invlpg (%eax)

    mov    %r11, %rdi
    mov    %r12, %rax

    # Setup some stack
    movq $(first_kernel_stack_end - 8), %rsp
    movq %rsp, %rbp

    # jump into kernel address space
    jmpq *%rax

1:
    hlt
    jmp 1b

stack_size = 0x10000

.section .bss.stack, "aw"
.align 4096
first_kernel_stack:
.space stack_size
first_kernel_stack_end:
