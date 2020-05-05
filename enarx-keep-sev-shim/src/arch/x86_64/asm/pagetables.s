.macro  QUAD from,count,step
.set    offset,0
.rept   \count
.quad   (\from + offset)
.set    offset,offset+\step
.endr
.endm

.section .pmldata, "aw"

.global PML4T
.align 4096
PML4T:
.quad (PML3IDENT + 0b00000111)
.space 120
.quad (PML3TO + 0b00000111)
.space (4096-120-2)

.global PML3IDENT
.align 4096
PML3IDENT:
.quad (PML2IDENT + 0b00000111)
.space 4088

.global PML3TO
.align 4096
PML3TO:
QUAD  0x83,512,0x40000000

.global PML2IDENT
.align 4096
PML2IDENT:
.quad 0x00000083
.quad 0x000200083
.space 4080
