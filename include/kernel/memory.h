#ifndef HYBRIDOS_MEMORY_H
#define HYBRIDOS_MEMORY_H

#include "kernel/types.h"

#define PAGE_SIZE 4096
#define PAGE_MASK (~(PAGE_SIZE - 1))

/* Memory Protections */
#define PROT_NONE               0x0
#define PROT_READ               0x1
#define PROT_WRITE              0x2
#define PROT_EXEC               0x4

#define MAP_SHARED              0x01
#define MAP_PRIVATE             0x02
#define MAP_ANONYMOUS           0x20

/* Windows Allocation Types */
#define MEM_COMMIT              0x00001000
#define MEM_RESERVE             0x00002000
#define MEM_RELEASE             0x00008000

/* Windows Protections */
#define PAGE_NOACCESS           0x01
#define PAGE_READONLY           0x02
#define PAGE_READWRITE          0x04
#define PAGE_EXECUTE            0x10
#define PAGE_EXECUTE_READ       0x20
#define PAGE_EXECUTE_READWRITE  0x40

void vmm_init(void);
virt_addr_t vmm_allocate_pages(size_t num_pages, u32 protection);
void vmm_free_pages(virt_addr_t addr, size_t num_pages);

#endif /* HYBRIDOS_MEMORY_H */
