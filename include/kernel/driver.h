#ifndef HYBRIDOS_DRIVER_H
#define HYBRIDOS_DRIVER_H

#include "kernel/types.h"

/*
 * =========================================================================
 * 1. Windows WDM Emulation Definitions
 * =========================================================================
 */

#define IRP_MJ_CREATE                   0x00
#define IRP_MJ_CLOSE                    0x02
#define IRP_MJ_READ                     0x03
#define IRP_MJ_WRITE                    0x04
#define IRP_MJ_DEVICE_CONTROL           0x0e
#define IRP_MJ_MAXIMUM_FUNCTION         0x1b

typedef struct _IO_STATUS_BLOCK {
    union {
        NTSTATUS Status;
        void* Pointer;
    };
    uintptr_t Information;
} IO_STATUS_BLOCK, *PIO_STATUS_BLOCK;

typedef struct _DEVICE_OBJECT DEVICE_OBJECT, *PDEVICE_OBJECT;
typedef struct _DRIVER_OBJECT DRIVER_OBJECT, *PDRIVER_OBJECT;

typedef struct _IRP {
    u8 MajorFunction;
    void* UserBuffer;
    size_t BufferLength;
    u64 FileOffset;
    u32 IoControlCode;
    IO_STATUS_BLOCK IoStatus;
    PDEVICE_OBJECT DeviceObject;
    void* SystemBuffer;
} IRP, *PIRP;

typedef NTSTATUS (*PDRIVER_DISPATCH)(PDEVICE_OBJECT DeviceObject, PIRP Irp);
typedef NTSTATUS (*PDRIVER_INITIALIZE)(PDRIVER_OBJECT DriverObject, void* RegistryPath);

struct _DRIVER_OBJECT {
    char DriverName[64];
    PDRIVER_INITIALIZE DriverInit;
    PDRIVER_DISPATCH MajorFunction[IRP_MJ_MAXIMUM_FUNCTION + 1];
    PDEVICE_OBJECT DeviceObject;
};

struct _DEVICE_OBJECT {
    char DeviceName[64];
    PDRIVER_OBJECT DriverObject;
    void* DeviceExtension;
    u32 DeviceType;
    u32 Characteristics;
    PDEVICE_OBJECT NextDevice;
};

/*
 * =========================================================================
 * 2. Linux file_operations Emulation Definitions
 * =========================================================================
 */

struct inode;
struct file;

typedef struct file_operations {
    i32 (*open)(struct inode* node, struct file* filp);
    i64 (*read)(struct file* filp, char* buf, size_t count, i64* ppos);
    i64 (*write)(struct file* filp, const char* buf, size_t count, i64* ppos);
    i64 (*unlocked_ioctl)(struct file* filp, u32 cmd, uintptr_t arg);
    i32 (*release)(struct inode* node, struct file* filp);
} file_operations_t;

typedef struct linux_cdev {
    char name[64];
    const file_operations_t* fops;
    u32 dev_major;
    u32 dev_minor;
} linux_cdev_t;

/*
 * =========================================================================
 * 3. The Vortex Driver Model (Unified Hardware Bridge)
 * =========================================================================
 */

typedef enum {
    DRIVER_TYPE_WINDOWS_WDM,
    DRIVER_TYPE_LINUX_OPS
} driver_origin_t;

typedef struct hybrid_driver {
    char name[64];
    driver_origin_t origin;
    union {
        DRIVER_OBJECT wdm_driver;
        linux_cdev_t  linux_driver;
    };
    struct hybrid_driver* next;
} hybrid_driver_t;

/* Vortex Driver Subsystem API */
void vdm_init(void);
NTSTATUS vdm_register_windows_driver(DRIVER_OBJECT* drv);
i32 vdm_register_linux_driver(linux_cdev_t* ldrv);
NTSTATUS vdm_dispatch_irp(PDEVICE_OBJECT dev, PIRP irp);
i64 vdm_dispatch_linux_call(const char* dev_name, u8 op, void* buf, size_t count, i64 offset, u32 ioctl_cmd);

#endif /* HYBRIDOS_DRIVER_H */
