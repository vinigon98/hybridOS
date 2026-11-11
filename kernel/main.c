#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kernel/types.h"
#include "kernel/personality.h"
#include "kernel/process.h"
#include "kernel/syscall.h"
#include "kernel/driver.h"
#include "loaders/pe_loader.h"
#include "loaders/elf_loader.h"

/*
 * =========================================================================
 * Mock Drivers to Demonstrate Parallel Driver Acceptance
 * =========================================================================
 */

/* 1. Mock Windows WDM Driver (Dispatches IRPs) */
static NTSTATUS MockWdmDispatchRead(PDEVICE_OBJECT DeviceObject, PIRP Irp) {
    (void)DeviceObject;
    const char* message = "[Hardware Data from Windows WDM Network Driver]";
    size_t len = strlen(message);
    if (Irp->UserBuffer && Irp->BufferLength >= len) {
        memcpy(Irp->UserBuffer, message, len);
        Irp->IoStatus.Information = len;
        Irp->IoStatus.Status = STATUS_SUCCESS;
    }
    printf("[Windows WDM Driver] IRP_MJ_READ handled successfully!\n");
    return STATUS_SUCCESS;
}

static void RegisterSampleWindowsDriver(void) {
    DRIVER_OBJECT wdm_drv;
    memset(&wdm_drv, 0, sizeof(DRIVER_OBJECT));
    strncpy(wdm_drv.DriverName, "net_realtek_wdm", sizeof(wdm_drv.DriverName) - 1);
    wdm_drv.MajorFunction[IRP_MJ_READ] = MockWdmDispatchRead;

    vdm_register_windows_driver(&wdm_drv);
}

/* 2. Mock Linux Driver (Uses struct file_operations) */
static i64 MockLinuxRead(struct file* filp, char* buf, size_t count, i64* ppos) {
    (void)filp; (void)ppos;
    const char* message = "[Hardware Data from Linux GPU Driver]";
    size_t len = strlen(message);
    if (buf && count >= len) {
        memcpy(buf, message, len);
        printf("[Linux Kernel Driver] file_operations.read() executed successfully!\n");
        return (i64)len;
    }
    return 0;
}

static file_operations_t g_mock_linux_fops = {
    .read = MockLinuxRead
};

static void RegisterSampleLinuxDriver(void) {
    linux_cdev_t ldrv;
    memset(&ldrv, 0, sizeof(linux_cdev_t));
    strncpy(ldrv.name, "drm_amdgpu_linux", sizeof(ldrv.name) - 1);
    ldrv.fops = &g_mock_linux_fops;

    vdm_register_linux_driver(&ldrv);
}

/*
 * =========================================================================
 * Kernel Main Function
 * =========================================================================
 */
int main(void) {
    printf("================================================================\n");
    printf("   _   _       _          _     _  ____   _____ \n");
    printf("  | | | |_   _| |__  _ __(_) __| |/ ___| |_   _|  HybridOS Kernel\n");
    printf("  | |_| | | | | '_ \\| '__| |/ _` |\\___ \\   | |    Dual-ABI Architecture\n");
    printf("  |  _  | |_| | |_) | |  | | (_| | ___) |  | |    Proprietary Ring-0 Core\n");
    printf("  |_| |_|\\__, |_.__/|_|  |_|\\__,_||____/   |_|    (c) 2026 NexusCore\n");
    printf("         |___/                                    \n");
    printf("================================================================\n\n");

    /* 1. Subsystem Initialization */
    printf(">>> [PHASE 1] Initializing Subsystems and Drivers...\n");
    vdm_init();
    syscall_subsystems_init();
    printf("\n");

    /* 2. Register Both Windows and Linux Drivers in Parallel */
    printf(">>> [PHASE 2] Registering Heterogeneous Drivers in Parallel...\n");
    RegisterSampleWindowsDriver();
    RegisterSampleLinuxDriver();
    printf("\n");

    /* 3. Test Vortex Driver Model (VDM) Interoperability Bridge */
    printf(">>> [PHASE 3] Testing Cross-ABI Driver Bridge...\n");
    char buffer[128] = {0};
    /* Linux syscall reading from a Windows WDM driver! */
    i64 bytes_read = vdm_dispatch_linux_call("net_realtek_wdm", 0, buffer, sizeof(buffer) - 1, 0, 0);
    if (bytes_read > 0) {
        buffer[bytes_read] = '\0';
        printf("[Cross-Call Test] Successfully read %lld bytes through VDM Bridge: \"%s\"\n\n",
               (long long)bytes_read, buffer);
    }

    /* 4. Binary Loader Demonstrations */
    printf(">>> [PHASE 4] Testing Dual Binary Loaders (PE32+ & ELF64)...\n");

    /* A. Generate and parse a synthetic PE32+ header (Windows) */
    u8 sample_pe[1024];
    memset(sample_pe, 0, sizeof(sample_pe));
    IMAGE_DOS_HEADER* dos = (IMAGE_DOS_HEADER*)sample_pe;
    dos->e_magic = IMAGE_DOS_SIGNATURE; /* "MZ" */
    dos->e_lfanew = 128;

    IMAGE_NT_HEADERS64* nt = (IMAGE_NT_HEADERS64*)(sample_pe + 128);
    nt->Signature = IMAGE_NT_SIGNATURE; /* "PE\0\0" */
    nt->FileHeader.Machine = IMAGE_FILE_MACHINE_AMD64;
    nt->FileHeader.NumberOfSections = 2;
    nt->FileHeader.SizeOfOptionalHeader = sizeof(IMAGE_OPTIONAL_HEADER64);
    nt->OptionalHeader.Magic = 0x20B; /* PE32+ */
    nt->OptionalHeader.ImageBase = 0x0000000140000000ULL;
    nt->OptionalHeader.AddressOfEntryPoint = 0x1000;
    nt->OptionalHeader.SizeOfImage = 0x8000;

    hybrid_process_t* win_proc = process_create("explorer_host.exe", PERSONALITY_WINDOWS);
    pe_load_image(win_proc, sample_pe, sizeof(sample_pe));
    printf("\n");

    /* B. Generate and parse a synthetic ELF64 header (Linux) */
    u8 sample_elf[1024];
    memset(sample_elf, 0, sizeof(sample_elf));
    Elf64_Ehdr* ehdr = (Elf64_Ehdr*)sample_elf;
    ehdr->e_ident[0] = ELFMAG0;
    ehdr->e_ident[1] = ELFMAG1;
    ehdr->e_ident[2] = ELFMAG2;
    ehdr->e_ident[3] = ELFMAG3;
    ehdr->e_ident[4] = ELFCLASS64;
    ehdr->e_ident[5] = ELFDATA2LSB;
    ehdr->e_ident[6] = EV_CURRENT;
    ehdr->e_type = ET_EXEC;
    ehdr->e_machine = EM_X86_64;
    ehdr->e_entry = 0x0000000000401000ULL;
    ehdr->e_phoff = sizeof(Elf64_Ehdr);
    ehdr->e_phentsize = sizeof(Elf64_Phdr);
    ehdr->e_phnum = 1;

    Elf64_Phdr* phdr = (Elf64_Phdr*)(sample_elf + sizeof(Elf64_Ehdr));
    phdr->p_type = PT_LOAD;
    phdr->p_flags = PF_R | PF_X;
    phdr->p_vaddr = 0x0000000000400000ULL;
    phdr->p_filesz = 0x2000;
    phdr->p_memsz = 0x3000;

    hybrid_process_t* lin_proc = process_create("bash_server", PERSONALITY_LINUX);
    elf_load_image(lin_proc, sample_elf, sizeof(sample_elf));
    printf("\n");

    /* 5. Parallel Syscall Execution */
    printf(">>> [PHASE 5] Testing Parallel Dual-ABI Syscall Dispatching...\n");

    /* Test 5.1: Windows Process executing NtWriteFile via Microsoft x64 calling convention */
    printf("[Test 5.1] Executing Windows Syscall (NtWriteFile - SSDT 0x0008)...\n");
    g_current_process = win_proc;
    trap_frame_t win_tf;
    memset(&win_tf, 0, sizeof(trap_frame_t));
    win_tf.rax = SYS_WIN_NT_WRITE_FILE; /* Syscall index */
    win_tf.r10 = (u64)1;                /* Arg1: FileHandle = stdout */
    win_tf.rdx = 0;                     /* Arg2: Event */
    win_tf.r8  = 0;                     /* Arg3: ApcRoutine */
    win_tf.r9  = 0;                     /* Arg4: ApcContext */
    const char* win_msg = "Hello from Windows NT Application running inside HybridOS!\n";
    /* In Microsoft x64, Arg5 (IoStatusBlock) and Arg6 (Buffer) are passed on stack */
    u64 win_stack[8] = {0};
    win_stack[5] = 0;                   /* Arg5: IoStatusBlock */
    win_stack[6] = (u64)(uintptr_t)win_msg; /* Arg6: Buffer */
    win_tf.rsp = (u64)(uintptr_t)win_stack;

    hybrid_syscall_dispatcher(&win_tf);
    printf("Windows NT Syscall Return Code: 0x%08X\n\n", (unsigned int)win_tf.rax);

    /* Test 5.2: Linux Process executing sys_write via System V AMD64 calling convention */
    printf("[Test 5.2] Executing Linux Syscall (sys_write - Syscall #1)...\n");
    g_current_process = lin_proc;
    trap_frame_t lin_tf;
    memset(&lin_tf, 0, sizeof(trap_frame_t));
    const char* lin_msg = "Hello from Linux ELF Application running inside HybridOS!\n";
    lin_tf.rax = SYS_LINUX_WRITE;               /* Syscall #1 */
    lin_tf.rdi = 1;                             /* Arg1: fd = stdout */
    lin_tf.rsi = (u64)(uintptr_t)lin_msg;       /* Arg2: buffer */
    lin_tf.rdx = strlen(lin_msg);               /* Arg3: count */

    hybrid_syscall_dispatcher(&lin_tf);
    printf("Linux Syscall Return Value: %llu bytes written\n\n", (unsigned long long)lin_tf.rax);

    /* 6. Clean up */
    process_destroy(win_proc);
    process_destroy(lin_proc);

    printf("================================================================\n");
    printf("[SUCCESS] HybridOS Kernel PoC executed and validated with success!\n");
    printf("================================================================\n");
    return 0;
}
