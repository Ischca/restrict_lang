
        .section .data
        .msg:
            .string "Hello, World!"
        .section .text
        .globl main
        main:
            movl $4, %eax
            movl $1, %ebx
            leal .msg, %ecx
            movl $13, %edx
            int $0x80

            movl $1, %eax
            xorl %ebx, %ebx
            int $0x80
        