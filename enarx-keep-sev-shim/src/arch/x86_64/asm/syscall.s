# SPDX-License-Identifier: Apache-2.0

.section .text, "ax"
.global _syscall_enter
.type _syscall_enter, @function
.code64

XSAVE_STACK_OFFSET = 1088+64

.p2align 4
_syscall_enter:
    swapgs                 # Set gs segment to TSS
    mov    %rsp,%gs:0x1c   # Save userspace rsp
    mov    %gs:0x4,%rsp    # Load kernel rsp
    pushq  $0x1b           # Push userspace data segment  ((gdt::USER_DATA_SEG << 3) | 3)
    pushq  %gs:0x1c        # Push userspace rsp
    movq   $0x0,%gs:0x1c   # Clear userspace rs
    push   %r11            # Push rflags stored in r11
    pushq  $0x23           # Push userspace code segment  ((gdt::USER_CODE_SEG << 3) | 3)
    push   %rcx            # Push userspace return pointer
    swapgs                 # Restore gs

    # xsave
    movq   %rax, %r11
    movq   %rdx, %rcx
    pushq  %rbx
    movq   %rsp, %rbx                          # save stack pointer before xsave

    subq   $XSAVE_STACK_OFFSET, %rsp
    andq   $(~(0x40-1)), %rsp

    # clear xsave header - Vol 1 - 13.4.2
    movq    $0,        512(%rsp)
    movq    $0,        520(%rsp)
    movq    $0,        528(%rsp)
    movq    $0,        536(%rsp)
    movq    $0,        544(%rsp)
    movq    $0,        552(%rsp)
    movq    $0,        560(%rsp)
    movq    $0,        568(%rsp)

    movl        $-1, %edx
    movl        $-1, %eax
    xsaveopt    (%rsp)
    pushq  %rbx                                # push stack pointer before xsave

    # dummy xrstor to clear XINUSE for the kernel
    movabs $XSAVEOPT_CLEAN,  %rax
    xrstor (%rax)

    movq   %r11, %rax
    movq   %rcx, %rdx
    # xsave end

    #FIXME: sti

    # SYSV:    rdi, rsi, rdx, rcx, r8, r9
    # SYSCALL: rdi, rsi, rdx, r10, r8, r9
    mov    %r10, %rcx

    pushq   %rdi
    pushq   %rsi
    pushq   %rdx
    pushq   %r10
    pushq   %r8
    pushq   %r9
    pushq   %rax

    callq  syscall_rust

    popq    %rcx
    popq    %r9
    popq    %r8
    popq    %r10
    popq    %rdx
    popq    %rsi
    popq    %rdi

    #FIXME: cli

    # xrstor
    movq   %rax, %r11
    movq   %rdx, %rcx

    popq  %rbx
    movl   $-1, %edx
    movl   $-1, %eax
    xrstor (%rsp)
    movq   %rbx, %rsp
    popq   %rbx

    movq   %r11, %rax
    movq   %rcx, %rdx
    # xrstor end

    # FIXME: want to protect the kernel against userspace?
    # https:#www.kernel.org/doc/Documentation/x86/entry_64.txt
    # use:
    iretq

    # FIXME: comment out iretq for fast return with sysretq
    //FIXME: cli
    swapgs
    pop    %rcx             # Pop userspace return pointer
    add    $0x8,%rsp        # Pop userspace code segment
    pop    %r11             # pop rflags to r11
    popq   %gs:0x1c         # Pop userspace rsp
    mov    %gs:0x1c,%rsp    # Restore userspace rsp
    swapgs
    sti
    sysretq


.section .data, "aw"

.global XSAVEOPT_CLEAN
.align 4096
XSAVEOPT_CLEAN:
.space 4096
