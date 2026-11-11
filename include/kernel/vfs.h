#ifndef HYBRIDOS_VFS_H
#define HYBRIDOS_VFS_H

#include "kernel/types.h"

#define VFS_FILE        0x01
#define VFS_DIRECTORY   0x02
#define VFS_CHARDEVICE  0x03
#define VFS_BLOCKDEVICE 0x04

typedef struct vfs_node {
    char name[128];
    u32 type;
    size_t size;
    u32 permissions;
    void* device_ptr; /* If device, pointer to hybrid_driver */
    
    /* Callbacks */
    i64 (*read)(struct vfs_node* node, u64 offset, size_t size, u8* buffer);
    i64 (*write)(struct vfs_node* node, u64 offset, size_t size, const u8* buffer);
    i32 (*open)(struct vfs_node* node, u32 flags);
    i32 (*close)(struct vfs_node* node);
    
    struct vfs_node* parent;
    struct vfs_node* next_sibling;
    struct vfs_node* first_child;
} vfs_node_t;

void vfs_init(void);
vfs_node_t* vfs_resolve_path(const char* path);
vfs_node_t* vfs_create_node(const char* path, u32 type);

#endif /* HYBRIDOS_VFS_H */
