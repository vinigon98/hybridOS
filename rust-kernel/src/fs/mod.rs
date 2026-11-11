//! HybridOS Filesystem & Security Filtering Subsystem (`fs`)
//!
//! Exposes the Early Filter Manager (`FLTMGR`), `VfsPreMountHook` engine,
//! and dynamic volume event notification (`EVT_VOLUME_MOUNTED`).

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
pub mod fltmgr;

pub use crate::compat::fs::*;

pub use fltmgr::{
    init_early_fltmgr, get_fltmgr, register_pre_mount_hook, trigger_pre_mount,
    notify_volume_mounted, is_volume_pre_coupled,
    EarlyFltMgr, FltError, MinifilterCallback, RegisteredMinifilter,
    VfsPreMountHook, VolumeAttachment, VolumeMountRequest, VolumeMountedEvent,
    ALTITUDE_FSFILTER_ACTIVITY_MONITOR, ALTITUDE_FSFILTER_ANTIVIRUS,
    ALTITUDE_FSFILTER_CONTINUOUS_BACKUP, ALTITUDE_FSFILTER_ENCRYPTION,
    EARLY_BASE_MAGIC, EARLY_FLTMGR_BASE_SIZE, EVT_VOLUME_MOUNTED,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Test 1: Bootstrap of Phase 1.5 Early FLTMGR allocates and initializes exactly 2 MB base.
    #[test]
    fn test_early_fltmgr_bootstrap_2mb_base() {
        let flt = EarlyFltMgr::bootstrap().expect("FLTMGR bootstrap failed");

        // Verify base memory size is exactly 2 MB (2 * 1024 * 1024 bytes)
        assert_eq!(flt.base_size(), 2 * 1024 * 1024);
        assert_eq!(flt.base_memory().len(), 2097152);

        // Verify magic signature and header integrity
        assert!(flt.verify_base_integrity());
        assert_eq!(&flt.base_memory()[0..16], EARLY_BASE_MAGIC);

        // Status flags check
        assert!(flt.is_active());

        // Default HyAV Kernel Gate filter must be present at altitude 320000
        assert!(flt.filter_count() >= 1);
    }

    /// Test 2: VfsPreMountHook pre-couples minifilter prior to volume mount completion.
    #[test]
    fn test_vfs_pre_mount_hook_prior_coupling() {
        let mut flt = EarlyFltMgr::bootstrap().expect("FLTMGR bootstrap failed");

        let hook_executed = Arc::new(AtomicBool::new(false));
        let hook_exec_clone = hook_executed.clone();

        // Register custom VfsPreMountHook
        flt.register_pre_mount_hook(Box::new(move |req: &VolumeMountRequest| {
            assert_eq!(req.mount_path, "/mnt/data");
            assert_eq!(req.fs_type, "FAT32");
            hook_exec_clone.store(true, Ordering::SeqCst);
            Ok(())
        }));

        let mount_req = VolumeMountRequest::new(101, "/mnt/data", "FAT32", 1);

        // Verify volume is NOT pre-coupled before trigger
        assert!(!flt.is_volume_pre_coupled("/mnt/data"));

        // Trigger pre-mount
        flt.trigger_pre_mount(&mount_req).expect("Pre-mount hook failed");

        // Verify hook executed
        assert!(hook_executed.load(Ordering::SeqCst));

        // Verify filter is now pre-coupled to volume PRIOR to finalizing mount!
        assert!(flt.is_volume_pre_coupled("/mnt/data"));

        let attachment = flt.get_volume_attachment("/mnt/data").expect("Missing attachment");
        assert!(attachment.pre_coupled);
        assert_eq!(attachment.fs_type, "FAT32");
        assert!(attachment.attached_filters.contains(&"HyAvKernelGateFilter".to_string()));
    }

    /// Test 3: EVT_VOLUME_MOUNTED dynamic volume notification broadcast.
    #[test]
    fn test_dynamic_volume_mounted_notification() {
        let mut flt = EarlyFltMgr::bootstrap().expect("FLTMGR bootstrap failed");

        struct TestListener {
            event_received: AtomicBool,
            received_event_name: std::sync::Mutex<String>,
        }

        impl MinifilterCallback for TestListener {
            fn on_volume_mounted(&self, event: &VolumeMountedEvent) {
                self.event_received.store(true, Ordering::SeqCst);
                let mut guard = self.received_event_name.lock().unwrap();
                *guard = event.event_name.to_string();
            }
        }

        let listener = Arc::new(TestListener {
            event_received: AtomicBool::new(false),
            received_event_name: std::sync::Mutex::new(String::new()),
        });

        // Register custom listener minifilter at altitude 360000 (Activity Monitor)
        flt.register_minifilter(
            "TestActivityMonitor",
            ALTITUDE_FSFILTER_ACTIVITY_MONITOR,
            listener.clone(),
        ).expect("Registration failed");

        let mount_req = VolumeMountRequest::new(202, "/boot/efi", "FAT32", 2);

        // 1. Pre-mount hook
        flt.trigger_pre_mount(&mount_req).unwrap();

        // 2. Dispatch dynamic EVT_VOLUME_MOUNTED
        let event = VolumeMountedEvent::new(202, "/boot/efi", "FAT32", vec![]);
        flt.notify_volume_mounted(event);

        // Verify listener was notified
        assert!(listener.event_received.load(Ordering::SeqCst));
        let event_name = listener.received_event_name.lock().unwrap().clone();
        assert_eq!(event_name, EVT_VOLUME_MOUNTED);

        // Verify event is logged in FLTMGR history
        assert_eq!(flt.dispatched_events().len(), 1);
        assert_eq!(flt.dispatched_events()[0].event_name, EVT_VOLUME_MOUNTED);
        assert_eq!(flt.dispatched_events()[0].mount_path, "/boot/efi");
    }

    /// Test 4: Pre-mount hook denial aborts volume mounting.
    #[test]
    fn test_pre_mount_hook_denial_blocks_mount() {
        let mut flt = EarlyFltMgr::bootstrap().expect("FLTMGR bootstrap failed");

        // Hook that rejects unapproved filesystem types
        flt.register_pre_mount_hook(Box::new(|req: &VolumeMountRequest| {
            if req.fs_type == "SUSPICIOUS_RAW" {
                return Err(FltError::AccessDenied);
            }
            Ok(())
        }));

        let denied_req = VolumeMountRequest::new(303, "/mnt/untrusted", "SUSPICIOUS_RAW", 3);
        let result = flt.trigger_pre_mount(&denied_req);

        // Must return AccessDenied
        assert_eq!(result, Err(FltError::AccessDenied));
        // Volume must NOT be pre-coupled or marked valid
        assert!(!flt.is_volume_pre_coupled("/mnt/untrusted"));
    }

    /// Test 5: Minifilters ordered by altitude descending.
    #[test]
    fn test_minifilters_ordered_by_altitude() {
        let mut flt = EarlyFltMgr::bootstrap().expect("FLTMGR bootstrap failed");

        struct DummyFilter;
        impl MinifilterCallback for DummyFilter {}

        flt.register_minifilter("LowAltFilter", 100000, Arc::new(DummyFilter)).unwrap();
        flt.register_minifilter("HighAltFilter", 400000, Arc::new(DummyFilter)).unwrap();
        flt.register_minifilter("MidAltFilter", 250000, Arc::new(DummyFilter)).unwrap();

        let mount_req = VolumeMountRequest::new(404, "/mnt/test_order", "EXT4", 4);
        flt.trigger_pre_mount(&mount_req).unwrap();

        let attachment = flt.get_volume_attachment("/mnt/test_order").unwrap();
        // HighAltFilter (400000) should appear before HyAvKernelGateFilter (320000)
        let high_pos = attachment.attached_filters.iter().position(|f| f == "HighAltFilter").unwrap();
        let av_pos = attachment.attached_filters.iter().position(|f| f == "HyAvKernelGateFilter").unwrap();
        let mid_pos = attachment.attached_filters.iter().position(|f| f == "MidAltFilter").unwrap();
        let low_pos = attachment.attached_filters.iter().position(|f| f == "LowAltFilter").unwrap();

        assert!(high_pos < av_pos);
        assert!(av_pos < mid_pos);
        assert!(mid_pos < low_pos);
    }
}
