/* RP2040 capacity map for a linked sizing probe, without boot/vector support. */
MEMORY {
  FLASH (rx)  : ORIGIN = 0x10000000, LENGTH = 2M
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 264K
}
ENTRY(_start)
SECTIONS {
  .text : {
    KEEP(*(.text.runtime_storage_bytes))
    KEEP(*(.text.core1_entry))
    *(.text .text.*)
  } > FLASH
  .rodata : {
    *(.rodata .rodata.*)
  } > FLASH
  .data : { *(.data .data.*) } > RAM AT> FLASH
  .bss (NOLOAD) : { *(.bss .bss.*) } > RAM
  /DISCARD/ : { *(.ARM.exidx .ARM.exidx.*) *(.ARM.extab .ARM.extab.*) }
}
