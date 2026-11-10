//! HybridOS Universal Graphics Subsystem
//!
//! Provides the complete Linux DRM/KMS graphics pipeline, VirtIO-GPU hardware accelerator,
//! 3D Vulkan/DirectX bridge, and desktop window compositor:
//! - [`drm`]: Direct Rendering Manager Core, DRM Minor nodes (`/dev/dri/card0`, `/dev/dri/renderD128`),
//!   GEM (Graphics Execution Manager) allocator, dumb buffer management, and DRM IOCTL dispatch table.
//! - [`kms`]: Kernel Mode Setting subsystem, Framebuffers, Planes (Primary/Overlay/Cursor), CRTCs,
//!   Encoders, Connectors, Display Modes (60Hz/144Hz), and Atomic Page Flipping with VBlank notification.
//! - [`virtio_gpu`]: VirtIO-GPU 2D/3D hardware device specification, Virtqueue ring buffer, and
//!   host scanout display processing.
//! - [`vulkan_hal`]: Vulkan ICD & Hardware Abstraction Layer (Vulkan 1.3, Queues, Swapchains, ICD Dispatch).
//! - [`dxgi_bridge`]: DirectX / DXGI Compatibility Bridge (DXGI Factory, Adapter, Swapchain, Zero-Copy DRM/GEM Direct3D 11/12 Surface Mapping).
//! - [`compositor`]: Dual-Personality Window Compositor (Win32 HWND + Linux Wayland `wl_surface` side-by-side).

pub mod drm;
pub mod kms;
pub mod virtio_gpu;
pub mod vulkan_hal;
pub mod dxgi_bridge;
pub mod compositor;

use std::sync::{Arc, Mutex};

// Re-exports: DRM subsystem
pub use drm::{
    calculate_aligned_size, calculate_pitch, DrmDevice, DrmError, DrmGemClose, DrmMinor,
    DrmMinorType, DrmModeCardRes, DrmModeCreateDumb, DrmModeCrtc, DrmModeCrtcPageFlip,
    DrmModeDestroyDumb, DrmModeFbCmd, DrmModeMapDumb, DrmVersion, GemAllocator, GemObject,
    DRM_IOCTL_GEM_CLOSE, DRM_IOCTL_GET_UNIQUE, DRM_IOCTL_MODE_ADDFB, DRM_IOCTL_MODE_CREATE_DUMB,
    DRM_IOCTL_MODE_DESTROY_DUMB, DRM_IOCTL_MODE_GETCRTC, DRM_IOCTL_MODE_GETRESOURCES,
    DRM_IOCTL_MODE_MAP_DUMB, DRM_IOCTL_MODE_PAGE_FLIP, DRM_IOCTL_MODE_RMFB, DRM_IOCTL_MODE_SETCRTC,
    DRM_IOCTL_VERSION, DRM_MAJOR,
};

// Re-exports: KMS subsystem
pub use kms::{
    ConnectorStatus, ConnectorType, DisplayMode, EncoderType, KmsConnector, KmsCrtc, KmsEncoder,
    KmsError, KmsFramebuffer, KmsPlane, KmsSubsystem, PageFlipRequest, PlaneType, VBlankEvent,
    DRM_FORMAT_ARGB8888, DRM_FORMAT_RGB565, DRM_FORMAT_XRGB8888, DRM_MODE_PAGE_FLIP_ASYNC,
    DRM_MODE_PAGE_FLIP_EVENT,
};

// Re-exports: VirtIO-GPU specification
pub use virtio_gpu::{
    VirtioGpuCommand, VirtioGpuCtrlHdr, VirtioGpuDevice, VirtioGpuDisplayOne, VirtioGpuMemEntry,
    VirtioGpuRect, VirtioGpuResourceAttachBacking, VirtioGpuResourceCreate2d,
    VirtioGpuResourceFlush, VirtioGpuRingBuffer, VirtioGpuSetScanout, VirtioGpuTransferToHost2d,
    VIRTIO_GPU_CMD_GET_DISPLAY_INFO, VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING,
    VIRTIO_GPU_CMD_RESOURCE_CREATE_2D, VIRTIO_GPU_CMD_RESOURCE_FLUSH, VIRTIO_GPU_CMD_SET_SCANOUT,
    VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D, VIRTIO_GPU_DEVICE_ID, VIRTIO_GPU_RESP_OK_DISPLAY_INFO,
    VIRTIO_GPU_RESP_OK_NODATA, VIRTIO_GPU_VENDOR_ID,
};

// Re-exports: Vulkan ICD HAL
pub use vulkan_hal::{
    VulkanIcd, VkPhysicalDevice, VkDevice, VkQueue, VkCommandBuffer, VkCommandPool,
    VkSwapchainKHR, VkSwapchainImage, VkSurfaceKHR, VkNativeSurfaceTarget,
    VkResult, VkFormat, VkColorSpaceKHR, VkPresentModeKHR,
};

// Re-exports: DXGI Bridge
pub use dxgi_bridge::{
    IDXGIFactory, IDXGIAdapter, IDXGIOutput, IDXGISwapChain, DrmGemBuffer, D3DResource,
    DXGI_FORMAT, DXGI_SWAP_CHAIN_DESC, DXGI_MODE_DESC, DXGI_RATIONAL, DXGI_SAMPLE_DESC,
    DXGI_SWAP_EFFECT, DXGI_USAGE_RENDER_TARGET_OUTPUT, HRESULT, S_OK,
};

// Re-exports: Window Compositor
pub use compositor::{
    UnifiedCompositor, CompositorSurface, SurfaceOrigin, DamageRegion, Rect, PixelBuffer,
    CursorPlane, CursorShape, WaylandRole, FrameRenderResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsError {
    InitializationFailed,
    DeviceNotFound,
    SwapchainAllocationFailed,
    SurfaceCreationFailed,
    LockError,
}

/// DRM & KMS Kernel Pipeline Driver Instance
#[derive(Debug)]
pub struct DrmKmsPipeline {
    pub drm_dev: DrmDevice,
    pub virtio_gpu: VirtioGpuDevice,
}

impl DrmKmsPipeline {
    pub fn new() -> Self {
        Self {
            drm_dev: DrmDevice::new(),
            virtio_gpu: VirtioGpuDevice::new(),
        }
    }

    /// Perform a full atomic page flip synchronized to the VirtIO-GPU scanout engine
    pub fn flip_and_sync_scanout(
        &mut self,
        crtc_id: u32,
        fb_id: u32,
        user_data: u64,
    ) -> Result<Option<VBlankEvent>, DrmError> {
        // 1. Queue KMS page flip with event requested
        self.drm_dev.kms.page_flip(
            crtc_id,
            fb_id,
            DRM_MODE_PAGE_FLIP_EVENT,
            user_data,
        )?;

        // 2. Fetch the newly bound framebuffer and backing GEM buffer
        let fb = self
            .drm_dev
            .kms
            .framebuffers
            .get(&fb_id)
            .ok_or(DrmError::InvalidArgument)?;
        let gem_handle = fb.gem_handles[0];
        let gem_obj = self
            .drm_dev
            .gem_allocator
            .get_object(gem_handle)
            .ok_or(DrmError::InvalidHandle(gem_handle))?;

        let (width, height, pixels) = {
            let locked = gem_obj.lock().unwrap();
            (locked.width, locked.height, locked.memory.clone())
        };

        // 3. Sync to VirtIO-GPU host scanout head 0
        let resource_id = fb_id;
        if !self.virtio_gpu.resources.contains_key(&resource_id) {
            let create_cmd = VirtioGpuResourceCreate2d {
                hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_CREATE_2D),
                resource_id,
                format: virtio_gpu::VIRTIO_GPU_FORMAT_B8G8R8X8_UNORM,
                width,
                height,
            };
            self.virtio_gpu.cmd_resource_create_2d(&create_cmd);
        }

        let transfer_cmd = VirtioGpuTransferToHost2d {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D),
            rect: VirtioGpuRect::new(0, 0, width, height),
            offset: 0,
            resource_id,
            padding: 0,
            pixel_data: pixels,
        };
        self.virtio_gpu.cmd_transfer_to_host_2d(&transfer_cmd);

        let set_scanout_cmd = VirtioGpuSetScanout {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_SET_SCANOUT),
            rect: VirtioGpuRect::new(0, 0, width, height),
            scanout_id: 0,
            resource_id,
        };
        self.virtio_gpu.cmd_set_scanout(&set_scanout_cmd);

        let flush_cmd = VirtioGpuResourceFlush {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_FLUSH),
            rect: VirtioGpuRect::new(0, 0, width, height),
            resource_id,
            padding: 0,
        };
        self.virtio_gpu.cmd_resource_flush(&flush_cmd);

        // 4. Trigger VBlank interrupt to conclude the frame
        let event = self.drm_dev.kms.trigger_vblank(crtc_id)?;
        Ok(event)
    }
}

/// Unified 3D Graphics Subsystem Controller
pub struct GraphicsSubsystem {
    pub vulkan_icd: Arc<Mutex<VulkanIcd>>,
    pub dxgi_factory: Arc<Mutex<IDXGIFactory>>,
    pub compositor: Arc<Mutex<UnifiedCompositor>>,
    pub drm_kms_pipeline: Arc<Mutex<DrmKmsPipeline>>,
    pub display_width: u32,
    pub display_height: u32,
    pub is_initialized: bool,
}

impl GraphicsSubsystem {
    pub fn new(width: u32, height: u32) -> Result<Self, GraphicsError> {
        let vulkan_icd = Arc::new(Mutex::new(VulkanIcd::new()));
        let dxgi_factory = Arc::new(Mutex::new(IDXGIFactory::new()));
        let compositor = Arc::new(Mutex::new(UnifiedCompositor::new(width, height)));
        let drm_kms_pipeline = Arc::new(Mutex::new(DrmKmsPipeline::new()));

        Ok(Self {
            vulkan_icd,
            dxgi_factory,
            compositor,
            drm_kms_pipeline,
            display_width: width,
            display_height: height,
            is_initialized: true,
        })
    }

    /// Creates a Windows Direct3D swapchain connected directly to a Compositor HWND surface
    pub fn create_win32_window_swapchain(
        &self,
        hwnd: usize,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(Arc<Mutex<IDXGISwapChain>>, Arc<Mutex<CompositorSurface>>), GraphicsError> {
        let mut comp = self.compositor.lock().map_err(|_| GraphicsError::LockError)?;
        let surf = comp.create_windows_surface(hwnd, title, x, y, width, height);

        let factory = self.dxgi_factory.lock().map_err(|_| GraphicsError::LockError)?;
        let swap_desc = DXGI_SWAP_CHAIN_DESC {
            buffer_desc: DXGI_MODE_DESC {
                width,
                height,
                refresh_rate: DXGI_RATIONAL { numerator: 60, denominator: 1 },
                format: DXGI_FORMAT::B8G8R8A8_UNORM,
                scanline_ordering: dxgi_bridge::DXGI_MODE_SCANLINE_ORDER::PROGRESSIVE,
                scaling: dxgi_bridge::DXGI_MODE_SCALING::UNSPECIFIED,
            },
            sample_desc: DXGI_SAMPLE_DESC { count: 1, quality: 0 },
            buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            buffer_count: 2,
            output_window: hwnd,
            windowed: true,
            swap_effect: DXGI_SWAP_EFFECT::FLIP_DISCARD,
            flags: 0,
        };

        let swapchain = factory.create_swap_chain(&swap_desc).map_err(|_| GraphicsError::SwapchainAllocationFailed)?;
        Ok((swapchain, surf))
    }

    /// Creates a Linux Wayland Vulkan swapchain connected directly to a Compositor wl_surface
    pub fn create_wayland_vulkan_swapchain(
        &self,
        surface_id: u32,
        app_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        role: WaylandRole,
    ) -> Result<(Arc<Mutex<VkSwapchainKHR>>, Arc<Mutex<CompositorSurface>>), GraphicsError> {
        let mut comp = self.compositor.lock().map_err(|_| GraphicsError::LockError)?;
        let surf = comp.create_wayland_surface(surface_id, app_id, x, y, width, height, role);

        let vk_surface = Arc::new(VkSurfaceKHR::new(surface_id as u64, VkNativeSurfaceTarget::WaylandSurface(surface_id), width, height));
        let swap_info = vulkan_hal::VkSwapchainCreateInfoKHR {
            surface: vk_surface,
            min_image_count: 2,
            image_format: VkFormat::B8G8R8A8Unorm,
            image_color_space: VkColorSpaceKHR::SrgbNonlinear,
            image_extent: [width, height],
            image_array_layers: 1,
            image_usage: vulkan_hal::VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            present_mode: VkPresentModeKHR::Fifo,
        };

        let vk_swapchain = VkSwapchainKHR::new(surface_id as u64, &swap_info)
            .map_err(|_| GraphicsError::SwapchainAllocationFailed)?;

        Ok((Arc::new(Mutex::new(vk_swapchain)), surf))
    }

    /// Presents a Windows DXGI backbuffer directly into the Compositor surface
    pub fn present_win32_surface(
        &self,
        swapchain: &Arc<Mutex<IDXGISwapChain>>,
        surface: &Arc<Mutex<CompositorSurface>>,
    ) -> Result<(), GraphicsError> {
        let mut sc = swapchain.lock().map_err(|_| GraphicsError::LockError)?;
        let cur_idx = sc.get_current_back_buffer_index();
        let res_arc = sc.get_buffer(cur_idx).map_err(|_| GraphicsError::SwapchainAllocationFailed)?;

        // Map buffer contents into compositor surface
        {
            let res = res_arc.lock().map_err(|_| GraphicsError::LockError)?;
            let gem = res.gem_buffer.lock().map_err(|_| GraphicsError::LockError)?;
            let mem = gem.backing_memory.lock().map_err(|_| GraphicsError::LockError)?;
            let mut surf = surface.lock().map_err(|_| GraphicsError::LockError)?;
            let mut pixel_buf = PixelBuffer::new(gem.width, gem.height);
            let copy_len = pixel_buf.data.len().min(mem.len());
            pixel_buf.data[..copy_len].copy_from_slice(&mem[..copy_len]);
            surf.attach_buffer(pixel_buf);
        }

        sc.present(1, 0);
        Ok(())
    }

    /// Presents a Linux Vulkan swapchain image directly into the Compositor surface
    pub fn present_vulkan_surface(
        &self,
        swapchain: &Arc<Mutex<VkSwapchainKHR>>,
        surface: &Arc<Mutex<CompositorSurface>>,
    ) -> Result<(), GraphicsError> {
        let mut sc = swapchain.lock().map_err(|_| GraphicsError::LockError)?;
        let cur_idx = sc.acquire_next_image(0).map_err(|_| GraphicsError::SwapchainAllocationFailed)?;
        let img_arc = sc.get_image(cur_idx).ok_or(GraphicsError::SwapchainAllocationFailed)?;

        {
            let img = img_arc.lock().map_err(|_| GraphicsError::LockError)?;
            let mut surf = surface.lock().map_err(|_| GraphicsError::LockError)?;
            let mut pixel_buf = PixelBuffer::new(img.width, img.height);
            let copy_len = pixel_buf.data.len().min(img.buffer.len());
            pixel_buf.data[..copy_len].copy_from_slice(&img.buffer[..copy_len]);
            surf.attach_buffer(pixel_buf);
        }

        sc.queue_present(cur_idx).map_err(|_| GraphicsError::SwapchainAllocationFailed)?;
        Ok(())
    }

    /// Renders a unified composited frame
    pub fn render_frame(&self) -> Result<FrameRenderResult, GraphicsError> {
        let mut comp = self.compositor.lock().map_err(|_| GraphicsError::LockError)?;
        Ok(comp.render_frame())
    }
}

/// Global Subsystem Initialization Entry Point
pub fn init_graphics() -> Result<GraphicsSubsystem, GraphicsError> {
    let width = 1024;
    let height = 768;
    GraphicsSubsystem::new(width, height)
}

// ============================================================================
// Comprehensive Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drm_minor_types_and_paths() {
        let primary = DrmMinor::new(DrmMinorType::Primary, 0);
        assert_eq!(primary.dev_path(), "/dev/dri/card0");
        assert_eq!(primary.major, 226);
        assert_eq!(primary.minor, 0);
        assert!(primary.allows_modesetting());
        assert!(primary.allows_rendering());

        let render = DrmMinor::new(DrmMinorType::Render, 0);
        assert_eq!(render.dev_path(), "/dev/dri/renderD128");
        assert_eq!(render.major, 226);
        assert_eq!(render.minor, 128);
        assert!(!render.allows_modesetting());
        assert!(render.allows_rendering());

        let control = DrmMinor::new(DrmMinorType::Control, 0);
        assert_eq!(control.dev_path(), "/dev/dri/controlD64");
        assert_eq!(control.minor, 64);
    }

    #[test]
    fn test_gem_dumb_buffer_allocator_and_pitch() {
        let mut allocator = GemAllocator::new();

        let pitch_1080p = calculate_pitch(1920, 32);
        assert_eq!(pitch_1080p, 7680);
        assert_eq!(pitch_1080p % 64, 0);

        let pitch_1366 = calculate_pitch(1366, 32);
        assert_eq!(pitch_1366, 5504);
        assert_eq!(pitch_1366 % 64, 0);

        let aligned_size = calculate_aligned_size(pitch_1080p, 1080);
        assert_eq!(aligned_size % 4096, 0);
        assert_eq!(aligned_size, 7680 * 1080);

        let obj_arc = allocator.allocate_dumb(1920, 1080, 32).expect("Dumb alloc");
        let handle = {
            let mut obj = obj_arc.lock().unwrap();
            assert_eq!(obj.handle, 1);
            assert_eq!(obj.width, 1920);
            assert_eq!(obj.height, 1080);
            assert_eq!(obj.pitch, 7680);
            assert!(obj.mmap_offset >= 0x7000_0000_0000);

            assert!(obj.write_pixel_xrgb(10, 20, 0x00FF_AA55));
            assert_eq!(obj.read_pixel_xrgb(10, 20), Some(0x00FF_AA55));
            obj.handle
        };

        let offset = obj_arc.lock().unwrap().mmap_offset;
        let looked_up = allocator.lookup_by_offset(offset).expect("Lookup by offset");
        assert_eq!(looked_up.lock().unwrap().handle, handle);

        assert!(allocator.close_handle(handle).is_ok());
        assert!(allocator.get_object(handle).is_none());
        assert!(allocator.lookup_by_offset(offset).is_none());
    }

    #[test]
    fn test_kms_display_modes_and_timings() {
        let m_60 = DisplayMode::mode_1080p_60();
        assert!(m_60.is_valid());
        assert_eq!(m_60.vrefresh, 60);
        assert_eq!(m_60.hdisplay, 1920);
        assert_eq!(m_60.vdisplay, 1080);
        assert_eq!(m_60.clock, 148_500);

        let m_144 = DisplayMode::mode_1080p_144();
        assert!(m_144.is_valid());
        assert_eq!(m_144.vrefresh, 144);
        assert_eq!(m_144.clock, 356_400);

        let m_1440p = DisplayMode::mode_1440p_144();
        assert!(m_1440p.is_valid());
        assert_eq!(m_1440p.hdisplay, 2560);
        assert_eq!(m_1440p.vdisplay, 1440);

        let m_4k = DisplayMode::mode_4k_60();
        assert!(m_4k.is_valid());
        assert_eq!(m_4k.hdisplay, 3840);
        assert_eq!(m_4k.vdisplay, 2160);

        let bw = m_60.bandwidth_mbps(32);
        assert!(bw > 3000.0 && bw < 5000.0);
    }

    #[test]
    fn test_kms_pipeline_entities_and_modesetting() {
        let mut kms = KmsSubsystem::new();
        assert!(!kms.crtcs.is_empty());
        assert!(!kms.planes.is_empty());
        assert!(!kms.connectors.is_empty());
        assert!(!kms.encoders.is_empty());

        let crtc_id = *kms.crtcs.keys().next().unwrap();
        let conn_id = *kms.connectors.keys().next().unwrap();

        let fb_id = kms.add_framebuffer(1920, 1080, 7680, 1).unwrap();
        assert!(kms.framebuffers.contains_key(&fb_id));

        let mode = DisplayMode::mode_1080p_60();
        let set_res = kms.set_crtc(crtc_id, Some(fb_id), 0, 0, Some(mode.clone()), &[conn_id]);
        assert!(set_res.is_ok());

        let crtc = kms.get_crtc(crtc_id).unwrap();
        assert!(crtc.enabled);
        assert_eq!(crtc.fb_id, Some(fb_id));
        assert_eq!(crtc.active_mode, Some(mode));

        let plane = kms.get_plane(crtc.primary_plane_id).unwrap();
        assert_eq!(plane.fb_id, Some(fb_id));
        assert_eq!(plane.crtc_w, 1920);
        assert_eq!(plane.crtc_h, 1080);
    }

    #[test]
    fn test_kms_atomic_page_flip_and_vblank() {
        let mut kms = KmsSubsystem::new();
        let crtc_id = *kms.crtcs.keys().next().unwrap();
        let conn_id = *kms.connectors.keys().next().unwrap();

        let fb1 = kms.add_framebuffer(1920, 1080, 7680, 1).unwrap();
        let fb2 = kms.add_framebuffer(1920, 1080, 7680, 2).unwrap();

        kms.set_crtc(
            crtc_id,
            Some(fb1),
            0,
            0,
            Some(DisplayMode::mode_1080p_60()),
            &[conn_id],
        )
        .unwrap();

        let flip_res = kms.page_flip(
            crtc_id,
            fb2,
            DRM_MODE_PAGE_FLIP_EVENT,
            0xDEAD_BEEF_CAFE,
        );
        assert!(flip_res.is_ok());

        assert_eq!(kms.get_crtc(crtc_id).unwrap().fb_id, Some(fb1));
        assert_eq!(kms.pop_vblank_event(), None);

        let vblank_res = kms.trigger_vblank(crtc_id).unwrap();
        assert!(vblank_res.is_some());
        let event = vblank_res.unwrap();
        assert_eq!(event.crtc_id, crtc_id);
        assert_eq!(event.sequence, 1);
        assert_eq!(event.user_data, 0xDEAD_BEEF_CAFE);

        assert_eq!(kms.get_crtc(crtc_id).unwrap().fb_id, Some(fb2));
        assert_eq!(kms.pop_vblank_event(), Some(event));
    }

    #[test]
    fn test_drm_ioctl_dispatch_table() {
        let mut drm_dev = DrmDevice::new();

        let mut ver_buf = [0u8; 12];
        let rc = drm_dev
            .dispatch_ioctl(DRM_IOCTL_VERSION, &mut ver_buf, DrmMinorType::Primary)
            .unwrap();
        assert_eq!(rc, 0);
        let major = i32::from_ne_bytes(ver_buf[0..4].try_into().unwrap());
        assert_eq!(major, 1);

        let mut unique_buf = [0u8; 32];
        drm_dev
            .dispatch_ioctl(DRM_IOCTL_GET_UNIQUE, &mut unique_buf, DrmMinorType::Primary)
            .unwrap();
        let unique_str = std::str::from_utf8(&unique_buf).unwrap();
        assert!(unique_str.starts_with("pci:0000:00:02.0"));

        let mut create_payload = [0u8; 32];
        create_payload[0..4].copy_from_slice(&1080u32.to_ne_bytes());
        create_payload[4..8].copy_from_slice(&1920u32.to_ne_bytes());
        create_payload[8..12].copy_from_slice(&32u32.to_ne_bytes());
        drm_dev
            .dispatch_ioctl(
                DRM_IOCTL_MODE_CREATE_DUMB,
                &mut create_payload,
                DrmMinorType::Primary,
            )
            .unwrap();

        let handle = u32::from_ne_bytes(create_payload[16..20].try_into().unwrap());
        let pitch = u32::from_ne_bytes(create_payload[20..24].try_into().unwrap());
        let size = u64::from_ne_bytes(create_payload[24..32].try_into().unwrap());

        assert_eq!(handle, 1);
        assert_eq!(pitch, 7680);
        assert!(size >= 7680 * 1080);

        let mut map_payload = [0u8; 16];
        map_payload[0..4].copy_from_slice(&handle.to_ne_bytes());
        drm_dev
            .dispatch_ioctl(
                DRM_IOCTL_MODE_MAP_DUMB,
                &mut map_payload,
                DrmMinorType::Primary,
            )
            .unwrap();
        let offset = u64::from_ne_bytes(map_payload[8..16].try_into().unwrap());
        assert!(offset >= 0x7000_0000_0000);

        let res = drm_dev.ioctl_get_resources(DrmMinorType::Primary).unwrap();
        assert!(!res.crtc_ids.is_empty());
        assert!(!res.connector_ids.is_empty());
        assert!(!res.encoder_ids.is_empty());

        let mut add_fb_cmd = DrmModeFbCmd {
            fb_id: 0,
            width: 1920,
            height: 1080,
            pitch,
            bpp: 32,
            depth: 24,
            handle,
        };
        drm_dev.ioctl_add_fb(&mut add_fb_cmd).unwrap();
        let fb_id = add_fb_cmd.fb_id;
        assert!(fb_id > 0);

        let crtc_id = res.crtc_ids[0];
        let set_crtc_cmd = DrmModeCrtc {
            crtc_id,
            fb_id,
            x: 0,
            y: 0,
            gamma_size: 256,
            mode_valid: 1,
            mode: Some(DisplayMode::mode_1080p_60()),
        };
        drm_dev
            .ioctl_set_crtc(&set_crtc_cmd, &res.connector_ids, DrmMinorType::Primary)
            .unwrap();

        let crtc_state = drm_dev.ioctl_get_crtc(crtc_id).unwrap();
        assert_eq!(crtc_state.fb_id, fb_id);
        assert_eq!(crtc_state.mode_valid, 1);

        let mut flip_payload = [0u8; 24];
        flip_payload[0..4].copy_from_slice(&crtc_id.to_ne_bytes());
        flip_payload[4..8].copy_from_slice(&fb_id.to_ne_bytes());
        flip_payload[8..12].copy_from_slice(&DRM_MODE_PAGE_FLIP_ASYNC.to_ne_bytes());
        flip_payload[12..16].copy_from_slice(&0u32.to_ne_bytes());
        flip_payload[16..24].copy_from_slice(&9999u64.to_ne_bytes());

        let flip_rc = drm_dev.dispatch_ioctl(
            DRM_IOCTL_MODE_PAGE_FLIP,
            &mut flip_payload,
            DrmMinorType::Primary,
        );
        assert_eq!(flip_rc.unwrap(), 0);

        let render_err = drm_dev.ioctl_set_crtc(&set_crtc_cmd, &[], DrmMinorType::Render);
        assert_eq!(render_err, Err(DrmError::PermissionDenied));

        let mut rmfb_payload = [0u8; 4];
        rmfb_payload[0..4].copy_from_slice(&fb_id.to_ne_bytes());
        drm_dev
            .dispatch_ioctl(DRM_IOCTL_MODE_RMFB, &mut rmfb_payload, DrmMinorType::Primary)
            .unwrap();

        let mut close_payload = [0u8; 4];
        close_payload[0..4].copy_from_slice(&handle.to_ne_bytes());
        drm_dev
            .dispatch_ioctl(DRM_IOCTL_GEM_CLOSE, &mut close_payload, DrmMinorType::Primary)
            .unwrap();
    }

    #[test]
    fn test_virtio_gpu_hardware_commands_and_ring_buffer() {
        let mut ring = VirtioGpuRingBuffer::new();

        let get_info = VirtioGpuCommand::GetDisplayInfo(VirtioGpuCtrlHdr::with_fence(
            VIRTIO_GPU_CMD_GET_DISPLAY_INFO,
            1,
        ));
        let resp = ring.submit_command(get_info);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_DISPLAY_INFO);
        assert_eq!(ring.device.last_completed_fence, 1);

        let create_2d = VirtioGpuCommand::ResourceCreate2d(VirtioGpuResourceCreate2d {
            hdr: VirtioGpuCtrlHdr::with_fence(VIRTIO_GPU_CMD_RESOURCE_CREATE_2D, 2),
            resource_id: 100,
            format: virtio_gpu::VIRTIO_GPU_FORMAT_B8G8R8A8_UNORM,
            width: 1920,
            height: 1080,
        });
        let resp = ring.submit_command(create_2d);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_NODATA);
        assert_eq!(ring.device.last_completed_fence, 2);
        assert!(ring.device.resources.contains_key(&100));

        let attach = VirtioGpuCommand::ResourceAttachBacking(VirtioGpuResourceAttachBacking {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING),
            resource_id: 100,
            nr_entries: 1,
            entries: vec![VirtioGpuMemEntry {
                addr: 0x1_0000_0000,
                length: 1920 * 1080 * 4,
                padding: 0,
            }],
        });
        let resp = ring.submit_command(attach);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_NODATA);

        let set_scanout = VirtioGpuCommand::SetScanout(VirtioGpuSetScanout {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_SET_SCANOUT),
            rect: VirtioGpuRect::new(0, 0, 1920, 1080),
            scanout_id: 0,
            resource_id: 100,
        });
        let resp = ring.submit_command(set_scanout);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_NODATA);
        assert!(ring.device.scanouts[0].enabled);
        assert_eq!(ring.device.scanouts[0].resource_id, Some(100));

        let pixel_bytes = vec![0xFF, 0x00, 0x00, 0xFF];
        let transfer = VirtioGpuCommand::TransferToHost2d(VirtioGpuTransferToHost2d {
            hdr: VirtioGpuCtrlHdr::new(VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D),
            rect: VirtioGpuRect::new(5, 5, 1, 1),
            offset: 0,
            resource_id: 100,
            padding: 0,
            pixel_data: pixel_bytes,
        });
        let resp = ring.submit_command(transfer);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_NODATA);

        let flush = VirtioGpuCommand::ResourceFlush(VirtioGpuResourceFlush {
            hdr: VirtioGpuCtrlHdr::with_fence(VIRTIO_GPU_CMD_RESOURCE_FLUSH, 3),
            rect: VirtioGpuRect::new(0, 0, 1920, 1080),
            resource_id: 100,
            padding: 0,
        });
        let resp = ring.submit_command(flush);
        assert_eq!(resp.hdr_type, VIRTIO_GPU_RESP_OK_NODATA);
        assert_eq!(ring.device.total_flushes, 1);
        assert_eq!(ring.device.last_completed_fence, 3);

        let pixel = ring.device.read_scanout_pixel(0, 5, 5);
        assert_eq!(pixel, Some(0xFF00_00FF));
    }

    #[test]
    fn test_end_to_end_graphics_pipeline_sync() {
        let mut pipe = DrmKmsPipeline::new();

        let mut create_req = DrmModeCreateDumb {
            width: 1920,
            height: 1080,
            bpp: 32,
            flags: 0,
            handle: 0,
            pitch: 0,
            size: 0,
        };
        pipe.drm_dev.ioctl_create_dumb(&mut create_req).unwrap();
        let handle = create_req.handle;

        {
            let obj = pipe.drm_dev.gem_allocator.get_object(handle).unwrap();
            let mut locked = obj.lock().unwrap();
            locked.write_pixel_xrgb(100, 100, 0x0000_FF00);
        }

        let mut fb_cmd = DrmModeFbCmd {
            fb_id: 0,
            width: 1920,
            height: 1080,
            pitch: create_req.pitch,
            bpp: 32,
            depth: 24,
            handle,
        };
        pipe.drm_dev.ioctl_add_fb(&mut fb_cmd).unwrap();
        let fb_id = fb_cmd.fb_id;

        let crtc_id = *pipe.drm_dev.kms.crtcs.keys().next().unwrap();
        let conn_id = *pipe.drm_dev.kms.connectors.keys().next().unwrap();
        let set_crtc = DrmModeCrtc {
            crtc_id,
            fb_id,
            x: 0,
            y: 0,
            gamma_size: 256,
            mode_valid: 1,
            mode: Some(DisplayMode::mode_1080p_60()),
        };
        pipe.drm_dev
            .ioctl_set_crtc(&set_crtc, &[conn_id], DrmMinorType::Primary)
            .unwrap();

        let event = pipe
            .flip_and_sync_scanout(crtc_id, fb_id, 0x1234_5678)
            .unwrap();
        assert!(event.is_some());
        let ev = event.unwrap();
        assert_eq!(ev.user_data, 0x1234_5678);

        assert!(pipe.virtio_gpu.scanouts[0].enabled);
        let pixel = pipe.virtio_gpu.read_scanout_pixel(0, 100, 100);
        assert_eq!(pixel, Some(0x0000_FF00));
    }

    #[test]
    fn test_init_graphics() {
        let graphics = init_graphics().expect("init_graphics should succeed");
        assert!(graphics.is_initialized);
        assert_eq!(graphics.display_width, 1024);
        assert_eq!(graphics.display_height, 768);

        let icd = graphics.vulkan_icd.lock().unwrap();
        assert_eq!(icd.physical_devices.len(), 1);
        assert_eq!(icd.physical_devices[0].properties.device_name, "HybridOS Vortex Universal 3D Accelerator (HAL)");

        let factory = graphics.dxgi_factory.lock().unwrap();
        let adapter = factory.enum_adapters(0).expect("Default DXGI adapter");
        assert_eq!(adapter.get_desc().description, "HybridOS Vortex Virtual GPU Direct3D 11/12 Bridge");
    }

    #[test]
    fn test_end_to_end_parallel_graphics_pipeline() {
        let graphics = init_graphics().expect("Graphics subsystem initialization");

        let (win_swapchain, win_surface) = graphics.create_win32_window_swapchain(
            0x1234,
            "Cyberpunk2077.exe",
            50,
            50,
            400,
            300,
        ).expect("Windows DXGI swapchain creation");

        {
            let sc = win_swapchain.lock().unwrap();
            let buf = sc.get_buffer(0).unwrap();
            let blue_pixel = [0xFF, 0x00, 0x00, 0xFF];
            let raw_data = blue_pixel.repeat(400 * 300);
            buf.lock().unwrap().write_pixels(&raw_data).unwrap();
        }
        graphics.present_win32_surface(&win_swapchain, &win_surface).unwrap();

        let (vk_swapchain, vk_surface) = graphics.create_wayland_vulkan_swapchain(
            42,
            "blender-vulkan",
            500,
            50,
            400,
            300,
            WaylandRole::Toplevel,
        ).expect("Linux Vulkan Wayland swapchain creation");

        {
            let sc = vk_swapchain.lock().unwrap();
            let img = sc.get_image(0).unwrap();
            let yellow_pixel = [0x00, 0xFF, 0xFF, 0xFF];
            let raw_data = yellow_pixel.repeat(400 * 300);
            img.lock().unwrap().buffer.copy_from_slice(&raw_data);
        }
        graphics.present_vulkan_surface(&vk_swapchain, &vk_surface).unwrap();

        let frame_result = graphics.render_frame().expect("Frame render");
        assert_eq!(frame_result.frame_index, 1);
        assert_eq!(frame_result.visible_windows, 2);
        assert!(frame_result.damaged_pixels > 0);

        let comp = graphics.compositor.lock().unwrap();

        // Windows surface at (150, 150) must be Blue
        let win_px = comp.get_pixel(150, 150).expect("Windows surface pixel");
        assert_eq!(win_px[0], 0xFF);
        assert_eq!(win_px[1], 0x00);
        assert_eq!(win_px[2], 0x00);

        // Linux surface at (600, 150) must be Yellow
        let lin_px = comp.get_pixel(600, 150).expect("Linux surface pixel");
        assert_eq!(lin_px[0], 0x00);
        assert_eq!(lin_px[1], 0xFF);
        assert_eq!(lin_px[2], 0xFF);

        // Cursor tip at (100, 100) on top of Windows surface
        let cursor_px = comp.get_pixel(100, 100).expect("Cursor pixel");
        assert_eq!(cursor_px[0], 0x00); // Black cursor outline
        assert_eq!(cursor_px[1], 0x00);
        assert_eq!(cursor_px[2], 0x00);

        let bg_px = comp.get_pixel(10, 10).expect("Background pixel");
        assert_eq!(bg_px, comp.background_color);
    }
}
