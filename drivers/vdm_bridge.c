#include "kernel/driver.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static hybrid_driver_t* g_driver_list_head = NULL;

void vdm_init(void) {
    g_driver_list_head = NULL;
    printf("[VDM] Vortex Driver Model Subsystem initialized.\n");
}

NTSTATUS vdm_register_windows_driver(DRIVER_OBJECT* drv) {
    if (!drv) return STATUS_INVALID_PARAMETER;

    hybrid_driver_t* entry = (hybrid_driver_t*)calloc(1, sizeof(hybrid_driver_t));
    entry->origin = DRIVER_TYPE_WINDOWS_WDM;
    strncpy(entry->name, drv->DriverName, sizeof(entry->name) - 1);
    memcpy(&entry->wdm_driver, drv, sizeof(DRIVER_OBJECT));

    entry->next = g_driver_list_head;
    g_driver_list_head = entry;

    printf("[VDM] Registered Windows WDM Driver: '%s' (Bridge: Exposed as /dev/%s for Linux)\n",
           entry->name, entry->name);

    return STATUS_SUCCESS;
}

i32 vdm_register_linux_driver(linux_cdev_t* ldrv) {
    if (!ldrv) return -EINVAL;

    hybrid_driver_t* entry = (hybrid_driver_t*)calloc(1, sizeof(hybrid_driver_t));
    entry->origin = DRIVER_TYPE_LINUX_OPS;
    strncpy(entry->name, ldrv->name, sizeof(entry->name) - 1);
    memcpy(&entry->linux_driver, ldrv, sizeof(linux_cdev_t));

    entry->next = g_driver_list_head;
    g_driver_list_head = entry;

    printf("[VDM] Registered Linux Kernel Driver: '%s' (Bridge: Exposed as \\Device\\%s for Windows NT)\n",
           entry->name, entry->name);

    return 0;
}

NTSTATUS vdm_dispatch_irp(PDEVICE_OBJECT dev, PIRP irp) {
    if (!dev || !irp) return STATUS_INVALID_PARAMETER;

    PDRIVER_OBJECT drv = dev->DriverObject;
    if (!drv) return STATUS_INVALID_PARAMETER;

    u8 mj = irp->MajorFunction;
    if (mj <= IRP_MJ_MAXIMUM_FUNCTION && drv->MajorFunction[mj]) {
        return drv->MajorFunction[mj](dev, irp);
    }

    return STATUS_NOT_IMPLEMENTED;
}

i64 vdm_dispatch_linux_call(const char* dev_name, u8 op, void* buf, size_t count, i64 offset, u32 ioctl_cmd) {
    (void)ioctl_cmd;
    hybrid_driver_t* curr = g_driver_list_head;
    while (curr) {
        if (strcmp(curr->name, dev_name) == 0) {
            if (curr->origin == DRIVER_TYPE_LINUX_OPS) {
                if (op == 0 && curr->linux_driver.fops && curr->linux_driver.fops->read) {
                    return curr->linux_driver.fops->read(NULL, (char*)buf, count, &offset);
                } else if (op == 1 && curr->linux_driver.fops && curr->linux_driver.fops->write) {
                    return curr->linux_driver.fops->write(NULL, (const char*)buf, count, &offset);
                }
            } else if (curr->origin == DRIVER_TYPE_WINDOWS_WDM) {
                /*
                 * THE BRIDGE: Translates Linux POSIX read/write into Windows NT IRP!
                 */
                printf("[VDM Bridge] Translating Linux operation to Windows IRP on driver '%s'...\n", curr->name);
                IRP irp;
                memset(&irp, 0, sizeof(IRP));
                irp.MajorFunction = (op == 0) ? IRP_MJ_READ : IRP_MJ_WRITE;
                irp.UserBuffer = buf;
                irp.BufferLength = count;
                irp.FileOffset = (u64)offset;

                NTSTATUS status = STATUS_SUCCESS;
                if (curr->wdm_driver.MajorFunction[irp.MajorFunction]) {
                    status = curr->wdm_driver.MajorFunction[irp.MajorFunction](NULL, &irp);
                }

                if (NT_SUCCESS(status)) {
                    return (i64)irp.IoStatus.Information;
                }
                return -EIO;
            }
        }
        curr = curr->next;
    }
    return -ENODEV;
}
