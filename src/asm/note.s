.section .note, "a"

# From xen/include/public/elfnote.h, "Physical entry point into the kernel."
XEN_ELFNOTE_PHYS32_ENTRY = 18

# We don't bother defining an ELFNOTE macro, as we only have one note.
# This is equialent to the kernel's:
# ELFNOTE(Xen, XEN_ELFNOTE_PHYS32_ENTRY, .long pvh_start)
.align 4
    .long name_end - name_start    # namesz
    .long desc_end - desc_start    # descsz
    .long XEN_ELFNOTE_PHYS32_ENTRY # type
name_start:
    .asciz "Xen"
name_end:
.align 4
desc_start:
    .long ram32_start
desc_end:
.align 4
