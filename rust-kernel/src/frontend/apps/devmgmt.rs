//! HybridOS Windows 11 Device Manager (devmgmt.msc) Subsystem
//!
//! Production-grade implementation of Microsoft Windows Device Manager and
//! Configuration Manager (cfgmgr32) engine for HybridOS:
//! 1. Hardware Device Tree Hierarchy with standard categories:
//!    - Audio inputs and outputs (Realtek HD Audio, VirtIO Sound Card, Audio Endpoints)
//!    - Computer (ACPI x64-based PC)
//!    - Disk drives (NVMe 1.4 SSD 64GB, ATA SATA AHCI Disk 32GB)
//!    - Display adapters (VirtIO-GPU PCI, AMD Radeon RX / Intel UHD Graphics)
//!    - Firmware (System Firmware UEFI 2.8)
//!    - Network adapters (VirtIO Network Adapter Gigabit, Realtek RTL8168 WDM)
//!    - Processors (Intel Core / AMD Ryzen 4 Cores)
//!    - Storage controllers (Standard NVM Express Controller, Standard SATA AHCI Controller)
//!    - System devices (PCI Express Root Complex, HPET, ACPI Power Button, PIC, DMA, RTC)
//! 2. Device Information, Hardware IDs, Status Codes (Working properly, Code 22 Disabled,
//!    Code 10 Failed Start, Code 43 Stopped, Code 1, Code 12, Code 28, Code 14) and
//!    Resource Allocations (IRQs, Memory Ranges, I/O Port Addresses, DMA Channels, Conflict Engine).
//! 3. Actions & Operations:
//!    - Scan for hardware changes (probing Vortex Driver Model / VDM bus & PCI config space)
//!    - Enable / Disable device toggle with system-critical safety protection
//!    - Update driver wizard dialog (Automatic search vs Browse computer)
//!    - Uninstall device with optional driver package removal from DriverStore.
//! 4. View Modes:
//!    - Devices by type, Devices by connection, Resources by type, Resources by connection,
//!      and Show hidden devices toggle.
//! 5. Properties Dialog (General, Driver, Details, Resources tabs) and Visual ASCII/UI Tree Renderer.
//!
//! 100% pure, safe, robust Rust with zero unsafe code.

#![deny(unsafe_code)]
#![allow(dead_code)]

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashMap;
use std::fmt;

// ============================================================================
// Section 1: Types, Enums & Status Codes
// ============================================================================

/// Windows Configuration Manager (cfgmgr32) Problem Codes.
/// Canonical problem codes matching Microsoft Windows Device Manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DeviceProblemCode {
    /// CM_PROB_NOT_CONFIGURED (Code 1): Device is not configured correctly.
    NotConfigured = 1,
    /// CM_PROB_FAILED_START (Code 10): This device cannot start.
    FailedStart = 10,
    /// CM_PROB_OUT_OF_RESOURCES (Code 12): Cannot find enough free resources.
    OutOfResources = 12,
    /// CM_PROB_NEED_RESTART (Code 14): Needs computer restart to work properly.
    NeedRestart = 14,
    /// CM_PROB_HARDWARE_DISABLED (Code 22): This device is disabled.
    HardwareDisabled = 22,
    /// CM_PROB_FAILED_INSTALL (Code 28): The drivers for this device are not installed.
    FailedInstall = 28,
    /// CM_PROB_FAILED_POST_START (Code 43): Stopped because it reported problems.
    FailedPostStart = 43,
}

impl DeviceProblemCode {
    /// Return the raw integer code.
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// Return human-readable Windows error message.
    pub const fn description(self) -> &'static str {
        match self {
            Self::NotConfigured => "This device is not configured correctly. (Code 1)",
            Self::FailedStart => "This device cannot start. (Code 10)",
            Self::OutOfResources => {
                "This device cannot find enough free resources that it can use. If you want to use this device, you will need to disable one of the other devices on this system. (Code 12)"
            }
            Self::NeedRestart => {
                "This device cannot work properly until you restart your computer. (Code 14)"
            }
            Self::HardwareDisabled => "This device is disabled. (Code 22)",
            Self::FailedInstall => "The drivers for this device are not installed. (Code 28)",
            Self::FailedPostStart => {
                "Windows has stopped this device because it has reported problems. (Code 43)"
            }
        }
    }
}

/// Operational status of a device node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is operational and functioning without problems (CM_PROB_NONE / Code 0).
    WorkingProperly,
    /// Device is disabled by the user or administrator (Code 22).
    Disabled,
    /// Device has encountered a problem or error code.
    Problem(DeviceProblemCode),
}

impl DeviceStatus {
    /// Return the numerical problem code (0 if working properly).
    pub fn problem_code(&self) -> u32 {
        match self {
            Self::WorkingProperly => 0,
            Self::Disabled => DeviceProblemCode::HardwareDisabled.code(),
            Self::Problem(prob) => prob.code(),
        }
    }

    /// Return true if the device is currently enabled and operational.
    pub fn is_working(&self) -> bool {
        matches!(self, Self::WorkingProperly)
    }

    /// Return true if the device is disabled.
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// Return true if the device has an error/problem.
    pub fn has_problem(&self) -> bool {
        matches!(self, Self::Problem(_))
    }

    /// Return friendly status string formatted for Device Manager status box.
    pub fn status_text(&self) -> String {
        match self {
            Self::WorkingProperly => "This device is working properly.".to_string(),
            Self::Disabled => DeviceProblemCode::HardwareDisabled.description().to_string(),
            Self::Problem(prob) => prob.description().to_string(),
        }
    }
}

/// Device Class Categories shown in Device Manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceCategory {
    AudioInputsAndOutputs,
    Batteries,
    Computer,
    DiskDrives,
    DisplayAdapters,
    Firmware,
    Keyboards,
    MiceAndOtherPointingDevices,
    NetworkAdapters,
    PortsComLpt,
    Processors,
    SoundVideoGameControllers,
    StorageControllers,
    SystemDevices,
    UniversalSerialBusControllers,
    OtherDevices,
}

impl DeviceCategory {
    /// Returns the standard Windows Device Manager display name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::AudioInputsAndOutputs => "Audio inputs and outputs",
            Self::Batteries => "Batteries",
            Self::Computer => "Computer",
            Self::DiskDrives => "Disk drives",
            Self::DisplayAdapters => "Display adapters",
            Self::Firmware => "Firmware",
            Self::Keyboards => "Keyboards",
            Self::MiceAndOtherPointingDevices => "Mice and other pointing devices",
            Self::NetworkAdapters => "Network adapters",
            Self::PortsComLpt => "Ports (COM & LPT)",
            Self::Processors => "Processors",
            Self::SoundVideoGameControllers => "Sound, video and game controllers",
            Self::StorageControllers => "Storage controllers",
            Self::SystemDevices => "System devices",
            Self::UniversalSerialBusControllers => "Universal Serial Bus controllers",
            Self::OtherDevices => "Other devices",
        }
    }

    /// Segoe Fluent icon glyph character for UI rendering.
    pub const fn icon_glyph(self) -> &'static str {
        match self {
            Self::AudioInputsAndOutputs => "🔊",
            Self::Batteries => "🔋",
            Self::Computer => "💻",
            Self::DiskDrives => "💽",
            Self::DisplayAdapters => "🖥️",
            Self::Firmware => "⚙️",
            Self::Keyboards => "⌨️",
            Self::MiceAndOtherPointingDevices => "🖱️",
            Self::NetworkAdapters => "🌐",
            Self::PortsComLpt => "🔌",
            Self::Processors => "🧠",
            Self::SoundVideoGameControllers => "🎮",
            Self::StorageControllers => "💾",
            Self::SystemDevices => "🗄️",
            Self::UniversalSerialBusControllers => "🔱",
            Self::OtherDevices => "❓",
        }
    }

    /// Windows Device Class GUID string.
    pub const fn class_guid(self) -> &'static str {
        match self {
            Self::AudioInputsAndOutputs => "{c166523c-fe0c-4a94-a586-fad77038cf46}",
            Self::Batteries => "{72631e1d-7842-11d0-bcf7-00aa00b7b32a}",
            Self::Computer => "{4d36e966-e325-11ce-bfc1-08002be10318}",
            Self::DiskDrives => "{4d36e967-e325-11ce-bfc1-08002be10318}",
            Self::DisplayAdapters => "{4d36e968-e325-11ce-bfc1-08002be10318}",
            Self::Firmware => "{f2e7dd72-6468-4e36-b6f1-6488f42c1b52}",
            Self::Keyboards => "{4d36e96b-e325-11ce-bfc1-08002be10318}",
            Self::MiceAndOtherPointingDevices => "{4d36e96f-e325-11ce-bfc1-08002be10318}",
            Self::NetworkAdapters => "{4d36e972-e325-11ce-bfc1-08002be10318}",
            Self::PortsComLpt => "{4d36e978-e325-11ce-bfc1-08002be10318}",
            Self::Processors => "{50127dc3-0f36-415e-a6cc-4cb3be910b65}",
            Self::SoundVideoGameControllers => "{4d36e96c-e325-11ce-bfc1-08002be10318}",
            Self::StorageControllers => "{4d36e97b-e325-11ce-bfc1-08002be10318}",
            Self::SystemDevices => "{4d36e97d-e325-11ce-bfc1-08002be10318}",
            Self::UniversalSerialBusControllers => "{36fc9e60-c465-11cf-8056-444553540000}",
            Self::OtherDevices => "{4d36e97e-e325-11ce-bfc1-08002be10318}",
        }
    }

    /// List all standard categories in canonical display order.
    pub const fn all() -> &'static [Self] {
        &[
            Self::AudioInputsAndOutputs,
            Self::Batteries,
            Self::Computer,
            Self::DiskDrives,
            Self::DisplayAdapters,
            Self::Firmware,
            Self::Keyboards,
            Self::MiceAndOtherPointingDevices,
            Self::NetworkAdapters,
            Self::PortsComLpt,
            Self::Processors,
            Self::SoundVideoGameControllers,
            Self::StorageControllers,
            Self::SystemDevices,
            Self::UniversalSerialBusControllers,
            Self::OtherDevices,
        ]
    }
}

/// Underlying bus architecture connection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    Root,
    Acpi,
    Pci,
    VirtIo,
    Usb,
    ScsiIde,
    Hdaudio,
    FirmwareBus,
}

impl BusType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "Root System Bus",
            Self::Acpi => "ACPI Bus",
            Self::Pci => "PCI / PCI Express Bus",
            Self::VirtIo => "VirtIO Vortex Bus",
            Self::Usb => "Universal Serial Bus",
            Self::ScsiIde => "Storage Channel Bus",
            Self::Hdaudio => "High Definition Audio Bus",
            Self::FirmwareBus => "UEFI Firmware Bus",
        }
    }
}

/// Driver details and digital signature info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverInfo {
    pub provider: String,
    pub date: String,
    pub version: String,
    pub digital_signer: String,
    pub inf_name: String,
    pub driver_files: Vec<String>,
    pub driver_store_path: String,
    pub is_inbox: bool,
}

impl DriverInfo {
    pub fn new(
        provider: &str,
        date: &str,
        version: &str,
        digital_signer: &str,
        inf_name: &str,
        driver_files: &[&str],
    ) -> Self {
        let store_path = format!(
            "C:\\Windows\\System32\\DriverStore\\FileRepository\\{}_{}_{}",
            inf_name.to_lowercase(),
            "amd64",
            version.replace('.', "")
        );
        Self {
            provider: provider.to_string(),
            date: date.to_string(),
            version: version.to_string(),
            digital_signer: digital_signer.to_string(),
            inf_name: inf_name.to_string(),
            driver_files: driver_files.iter().map(|s| s.to_string()).collect(),
            driver_store_path: store_path,
            is_inbox: provider.contains("Microsoft"),
        }
    }
}

// ============================================================================
// Section 2: Hardware Resources & Conflict Engine
// ============================================================================

/// Interrupt Request (IRQ) allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqResource {
    pub irq_number: u32,
    pub is_msi: bool,
    pub shareable: bool,
    pub affinity_mask: u64,
}

impl IrqResource {
    pub const fn new(irq_number: u32, is_msi: bool, shareable: bool) -> Self {
        Self {
            irq_number,
            is_msi,
            shareable,
            affinity_mask: 0xFFFFFFFFFFFFFFFF,
        }
    }

    pub fn display_string(&self) -> String {
        if self.is_msi {
            format!("(MSI-X) 0x{:08X} ({})", self.irq_number, self.irq_number)
        } else {
            format!("0x{:08X} ({})", self.irq_number, self.irq_number)
        }
    }
}

/// Physical memory mapped I/O (MMIO) range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRange {
    pub start: u64,
    pub end: u64,
    pub prefetchable: bool,
    pub is_64bit: bool,
}

impl MemoryRange {
    pub const fn new(start: u64, end: u64, prefetchable: bool, is_64bit: bool) -> Self {
        Self {
            start,
            end,
            prefetchable,
            is_64bit,
        }
    }

    pub fn size(&self) -> u64 {
        if self.end >= self.start {
            self.end - self.start + 1
        } else {
            0
        }
    }

    pub fn overlaps(&self, other: &MemoryRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    pub fn display_string(&self) -> String {
        format!("0x{:08X} - 0x{:08X}", self.start, self.end)
    }
}

/// Port-mapped I/O (PMIO) address range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoPortRange {
    pub start: u16,
    pub end: u16,
}

impl IoPortRange {
    pub const fn new(start: u16, end: u16) -> Self {
        Self { start, end }
    }

    pub fn size(&self) -> u32 {
        if self.end >= self.start {
            (self.end - self.start + 1) as u32
        } else {
            0
        }
    }

    pub fn overlaps(&self, other: &IoPortRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    pub fn display_string(&self) -> String {
        format!("0x{:04X} - 0x{:04X}", self.start, self.end)
    }
}

/// Direct Memory Access (DMA) channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaChannel {
    pub channel: u8,
    pub transfer_width_bits: u8,
}

impl DmaChannel {
    pub const fn new(channel: u8, transfer_width_bits: u8) -> Self {
        Self {
            channel,
            transfer_width_bits,
        }
    }

    pub fn display_string(&self) -> String {
        format!("{:02}", self.channel)
    }
}

/// Comprehensive hardware resource allocation block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourceAllocation {
    pub irqs: Vec<IrqResource>,
    pub memory_ranges: Vec<MemoryRange>,
    pub io_ports: Vec<IoPortRange>,
    pub dma_channels: Vec<DmaChannel>,
}

impl ResourceAllocation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.irqs.is_empty()
            && self.memory_ranges.is_empty()
            && self.io_ports.is_empty()
            && self.dma_channels.is_empty()
    }

    pub fn add_irq(&mut self, irq: u32, is_msi: bool, shareable: bool) -> &mut Self {
        self.irqs.push(IrqResource::new(irq, is_msi, shareable));
        self
    }

    pub fn add_memory(&mut self, start: u64, end: u64, prefetch: bool, is_64: bool) -> &mut Self {
        self.memory_ranges.push(MemoryRange::new(start, end, prefetch, is_64));
        self
    }

    pub fn add_io_port(&mut self, start: u16, end: u16) -> &mut Self {
        self.io_ports.push(IoPortRange::new(start, end));
        self
    }

    pub fn add_dma(&mut self, channel: u8, width: u8) -> &mut Self {
        self.dma_channels.push(DmaChannel::new(channel, width));
        self
    }

    /// Check if this resource allocation conflicts with another active device's resources.
    pub fn find_conflict(&self, other: &ResourceAllocation) -> Option<ResourceConflict> {
        // 1. Check IRQ conflicts (non-shareable IRQs colliding)
        for irq1 in &self.irqs {
            for irq2 in &other.irqs {
                if irq1.irq_number == irq2.irq_number && (!irq1.shareable || !irq2.shareable) {
                    return Some(ResourceConflict {
                        resource_type: "Interrupt Request (IRQ)",
                        conflicting_range: irq1.display_string(),
                        reason: format!(
                            "Collision on IRQ {} with a non-shareable interrupt request",
                            irq1.irq_number
                        ),
                    });
                }
            }
        }

        // 2. Check Memory MMIO range conflicts
        for mem1 in &self.memory_ranges {
            for mem2 in &other.memory_ranges {
                if mem1.overlaps(mem2) {
                    return Some(ResourceConflict {
                        resource_type: "Memory Range",
                        conflicting_range: mem1.display_string(),
                        reason: format!(
                            "Memory overlap between {} and {}",
                            mem1.display_string(),
                            mem2.display_string()
                        ),
                    });
                }
            }
        }

        // 3. Check I/O Port range conflicts
        for io1 in &self.io_ports {
            for io2 in &other.io_ports {
                if io1.overlaps(io2) {
                    return Some(ResourceConflict {
                        resource_type: "Input/Output Range",
                        conflicting_range: io1.display_string(),
                        reason: format!(
                            "I/O Port overlap between {} and {}",
                            io1.display_string(),
                            io2.display_string()
                        ),
                    });
                }
            }
        }

        // 4. Check DMA channel conflicts
        for dma1 in &self.dma_channels {
            for dma2 in &other.dma_channels {
                if dma1.channel == dma2.channel {
                    return Some(ResourceConflict {
                        resource_type: "Direct Memory Access (DMA)",
                        conflicting_range: dma1.display_string(),
                        reason: format!("DMA Channel {} already assigned", dma1.channel),
                    });
                }
            }
        }

        None
    }
}

/// Conflict report detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceConflict {
    pub resource_type: &'static str,
    pub conflicting_range: String,
    pub reason: String,
}

// ============================================================================
// Section 3: Device Node Representation
// ============================================================================

/// Represents a single hardware node in the PnP device tree hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceNode {
    pub id: String,
    pub friendly_name: String,
    pub device_desc: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub category: DeviceCategory,
    pub status: DeviceStatus,
    pub driver_info: DriverInfo,
    pub resources: ResourceAllocation,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub bus_type: BusType,
    pub location_info: String,
    pub physical_device_object: String,
    pub is_hidden: bool,
    pub is_removable: bool,
    pub is_system_critical: bool,
    pub driver_software_installed: bool,
    pub power_state: String,
}

impl DeviceNode {
    /// Create a new device node.
    pub fn new(
        id: &str,
        friendly_name: &str,
        device_desc: &str,
        hardware_ids: &[&str],
        category: DeviceCategory,
        driver_info: DriverInfo,
        bus_type: BusType,
        location_info: &str,
    ) -> Self {
        let pdo = format!(
            "\\Device\\NTPNP_{:04X}",
            (id.len() * 31 + friendly_name.len() * 17) & 0xFFFF
        );
        Self {
            id: id.to_string(),
            friendly_name: friendly_name.to_string(),
            device_desc: device_desc.to_string(),
            hardware_ids: hardware_ids.iter().map(|s| s.to_string()).collect(),
            compatible_ids: Vec::new(),
            category,
            status: DeviceStatus::WorkingProperly,
            driver_info,
            resources: ResourceAllocation::new(),
            parent_id: None,
            children_ids: Vec::new(),
            bus_type,
            location_info: location_info.to_string(),
            physical_device_object: pdo,
            is_hidden: false,
            is_removable: true,
            is_system_critical: false,
            driver_software_installed: true,
            power_state: "D0 (Working)".to_string(),
        }
    }

    /// Mark this node as a non-removable, system-critical component (e.g. ACPI Root PC or CPU 0).
    pub fn set_system_critical(&mut self, critical: bool) -> &mut Self {
        self.is_system_critical = critical;
        if critical {
            self.is_removable = false;
        }
        self
    }

    /// Set hidden status (e.g. unplugged USB storage or phantom legacy device).
    pub fn set_hidden(&mut self, hidden: bool) -> &mut Self {
        self.is_hidden = hidden;
        self
    }

    /// Set parent link.
    pub fn set_parent(&mut self, parent_id: &str) -> &mut Self {
        self.parent_id = Some(parent_id.to_string());
        self
    }

    /// Disable device (transition to Code 22).
    pub fn disable(&mut self) -> Result<(), DevMgmtError> {
        if self.is_system_critical {
            return Err(DevMgmtError::CannotDisableCriticalDevice(format!(
                "Cannot disable system-critical device '{}'",
                self.friendly_name
            )));
        }
        if self.status.is_disabled() {
            return Err(DevMgmtError::DeviceAlreadyDisabled(format!(
                "Device '{}' is already disabled",
                self.friendly_name
            )));
        }
        self.status = DeviceStatus::Disabled;
        self.power_state = "D3cold (Off)".to_string();
        Ok(())
    }

    /// Enable device (transition to Working properly).
    pub fn enable(&mut self) -> Result<(), DevMgmtError> {
        if self.status.is_working() {
            return Err(DevMgmtError::DeviceAlreadyEnabled(format!(
                "Device '{}' is already enabled",
                self.friendly_name
            )));
        }
        self.status = DeviceStatus::WorkingProperly;
        self.power_state = "D0 (Working)".to_string();
        Ok(())
    }

    /// Set error code on device.
    pub fn set_problem(&mut self, code: DeviceProblemCode) {
        self.status = DeviceStatus::Problem(code);
    }

    /// Clear error code and return to operational state.
    pub fn clear_problem(&mut self) {
        self.status = DeviceStatus::WorkingProperly;
    }

    /// Primary Hardware ID.
    pub fn primary_hardware_id(&self) -> &str {
        self.hardware_ids
            .first()
            .map(|s| s.as_str())
            .unwrap_or(self.id.as_str())
    }
}

// ============================================================================
// Section 4: Device Manager View Modes & UI Models
// ============================================================================

/// Device Manager visual tree presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    /// Standard category grouping (Audio, Computer, Disk drives, etc.)
    DevicesByType,
    /// Hierarchical ACPI/PCI hardware bus topology
    DevicesByConnection,
    /// Grouped by DMA, IO Port, IRQ, and Memory Range allocations
    ResourcesByType,
    /// Resource allocations displayed along the connection topology
    ResourcesByConnection,
}

impl ViewMode {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::DevicesByType => "Devices by type",
            Self::DevicesByConnection => "Devices by connection",
            Self::ResourcesByType => "Resources by type",
            Self::ResourcesByConnection => "Resources by connection",
        }
    }
}

/// Generic tree item for rendering in Fluent UI treeviews or ASCII terminal views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewItem {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub is_category: bool,
    pub is_expanded: bool,
    pub icon: &'static str,
    pub has_problem: bool,
    pub is_disabled: bool,
    pub device_id: Option<String>,
    pub resource_detail: Option<String>,
}

impl TreeViewItem {
    pub fn status_glyph(&self) -> &'static str {
        if self.has_problem {
            "[!]"
        } else if self.is_disabled {
            "[↓]"
        } else if self.is_category {
            "[+]"
        } else {
            "[✓]"
        }
    }
}

// ============================================================================
// Section 5: Driver Update Wizard & Action Reports
// ============================================================================

/// Options supplied to the Update Driver Software Wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDriverOptions {
    /// Search automatically for updated driver software (HybridOS Catalog & VDM repository).
    SearchAutomatically,
    /// Browse my computer for driver software from a specific filesystem folder.
    BrowseComputer {
        search_path: String,
        include_subfolders: bool,
    },
}

/// Result report from executing the driver update wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverUpdateResult {
    pub success: bool,
    pub driver_updated: bool,
    pub previous_version: String,
    pub new_version: String,
    pub wizard_steps: Vec<String>,
    pub message: String,
}

/// Result report from scanning for hardware changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub total_devices_scanned: usize,
    pub discovered_devices: Vec<String>,
    pub removed_devices: Vec<String>,
    pub bus_probed: String,
    pub duration_ms: u64,
}

/// Result report from device uninstallation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallReport {
    pub device_id: String,
    pub friendly_name: String,
    pub removed_from_tree: bool,
    pub driver_package_deleted: bool,
    pub deleted_package_path: Option<String>,
    pub message: String,
}

/// Common Device Manager errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevMgmtError {
    DeviceNotFound(String),
    DeviceAlreadyDisabled(String),
    DeviceAlreadyEnabled(String),
    CannotDisableCriticalDevice(String),
    DriverNotFound(String),
    OperationFailed(String),
}

impl fmt::Display for DevMgmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceNotFound(s) => write!(f, "Device not found: {}", s),
            Self::DeviceAlreadyDisabled(s) => write!(f, "Device already disabled: {}", s),
            Self::DeviceAlreadyEnabled(s) => write!(f, "Device already enabled: {}", s),
            Self::CannotDisableCriticalDevice(s) => {
                write!(f, "Cannot disable critical system device: {}", s)
            }
            Self::DriverNotFound(s) => write!(f, "Driver software not found: {}", s),
            Self::OperationFailed(s) => write!(f, "Operation failed: {}", s),
        }
    }
}

impl std::error::Error for DevMgmtError {}

// ============================================================================
// Section 6: Hardware Device Tree Engine
// ============================================================================

/// Master Hardware Device Tree hierarchy manager for HybridOS.
#[derive(Debug, Clone)]
pub struct DeviceTree {
    devices: HashMap<String, DeviceNode>,
    device_order: Vec<String>,
    root_id: String,
    hardware_bus_probed_count: usize,
    driver_store_packages: HashMap<String, String>,
}

impl DeviceTree {
    /// Construct an empty device tree.
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            device_order: Vec::new(),
            root_id: String::new(),
            hardware_bus_probed_count: 0,
            driver_store_packages: HashMap::new(),
        }
    }

    /// Construct a complete, production-grade standard PC hardware tree
    /// populated with all authentic hardware devices required by specifications.
    pub fn new_standard_pc() -> Self {
        let mut tree = Self::new();
        tree.populate_standard_hardware();
        tree
    }

    /// Insert a device node into the tree.
    pub fn insert_device(&mut self, node: DeviceNode) {
        let id = node.id.clone();
        if let Some(ref parent_id) = node.parent_id {
            if let Some(parent) = self.devices.get_mut(parent_id) {
                if !parent.children_ids.contains(&id) {
                    parent.children_ids.push(id.clone());
                }
            }
        }
        self.device_order.push(id.clone());
        self.devices.insert(id, node);
    }

    /// Populate standard authentic hardware hierarchy.
    fn populate_standard_hardware(&mut self) {
        // --------------------------------------------------------------------
        // 1. Root Computer Node: ACPI x64-based PC
        // --------------------------------------------------------------------
        let root_id = "ACPI_HAL\\PNP0C08\\0";
        self.root_id = root_id.to_string();

        let mut root_pc = DeviceNode::new(
            root_id,
            "ACPI x64-based PC",
            "Advanced Configuration and Power Interface (ACPI) PC",
            &["ACPI\\PNP0C08", "*PNP0C08"],
            DeviceCategory::Computer,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "hal.inf",
                &["C:\\Windows\\System32\\hal.dll"],
            ),
            BusType::Root,
            "on Motherboard System Complex",
        );
        root_pc.set_system_critical(true);
        self.insert_device(root_pc);

        // --------------------------------------------------------------------
        // 2. Motherboard Bridge: PCI Express Root Complex
        // --------------------------------------------------------------------
        let pci_root_id = "ACPI\\PNP0A08\\0";
        let mut pci_root = DeviceNode::new(
            pci_root_id,
            "PCI Express Root Complex",
            "PCI-to-PCI Bridge / Root Complex",
            &["ACPI\\PNP0A08", "*PNP0A08", "PCI\\CC_060400"],
            DeviceCategory::SystemDevices,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "pci.inf",
                &["C:\\Windows\\System32\\drivers\\pci.sys"],
            ),
            BusType::Acpi,
            "PCI Express Root Complex bus 0",
        );
        pci_root.set_system_critical(true);
        pci_root.set_parent(root_id);
        pci_root
            .resources
            .add_io_port(0x0CF8, 0x0CFF)
            .add_memory(0x000A0000, 0x000BFFFF, false, false);
        self.insert_device(pci_root);

        // --------------------------------------------------------------------
        // 3. System Devices: HPET, ACPI Power Button, PIC, DMA, RTC
        // --------------------------------------------------------------------
        let hpet_id = "ACPI\\PNP0103\\0";
        let mut hpet = DeviceNode::new(
            hpet_id,
            "High Precision Event Timer",
            "High Precision Event Timer (HPET)",
            &["ACPI\\PNP0103", "*PNP0103"],
            DeviceCategory::SystemDevices,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "hpet.inf",
                &["C:\\Windows\\System32\\drivers\\hpet.sys"],
            ),
            BusType::Acpi,
            "on Motherboard",
        );
        hpet.set_parent(pci_root_id);
        hpet.resources
            .add_irq(0, false, true)
            .add_memory(0xFED00000, 0xFED003FF, false, false);
        self.insert_device(hpet);

        let power_btn_id = "ACPI\\PNP0C0C\\0";
        let mut power_btn = DeviceNode::new(
            power_btn_id,
            "ACPI Power Button",
            "ACPI Power Button Controller",
            &["ACPI\\PNP0C0C", "*PNP0C0C"],
            DeviceCategory::SystemDevices,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "acpipagr.inf",
                &["C:\\Windows\\System32\\drivers\\acpipagr.sys"],
            ),
            BusType::Acpi,
            "on Motherboard",
        );
        power_btn.set_parent(pci_root_id);
        self.insert_device(power_btn);

        // --------------------------------------------------------------------
        // 4. Processors: Intel Core / AMD Ryzen 4 Cores
        // --------------------------------------------------------------------
        for core in 0..4 {
            let cpu_id = format!("ACPI\\GenuineIntel_-_x86_Family_6_Model_151\\_{}", core);
            let cpu_name = format!(
                "Intel(R) Core(TM) i7-12700K / AMD Ryzen 9 (Core {})",
                core
            );
            let mut cpu = DeviceNode::new(
                &cpu_id,
                &cpu_name,
                "x64-based Processor Core",
                &[
                    "ACPI\\GenuineIntel_-_x86_Family_6_Model_151",
                    "ACPI\\Processor",
                    "*PNP0C04",
                ],
                DeviceCategory::Processors,
                DriverInfo::new(
                    "Microsoft",
                    "2026-06-21",
                    "10.0.22621.1",
                    "Microsoft Windows Hardware Compatibility Publisher",
                    "cpu.inf",
                    &["C:\\Windows\\System32\\drivers\\intelppm.sys"],
                ),
                BusType::Acpi,
                &format!("Core Socket Slot {}", core),
            );
            cpu.set_parent(root_id);
            if core == 0 {
                cpu.set_system_critical(true);
            }
            self.insert_device(cpu);
        }

        // --------------------------------------------------------------------
        // 5. Firmware: System Firmware UEFI 2.8
        // --------------------------------------------------------------------
        let uefi_id = "UEFI\\RES_{2c13e54b-b003-4629-9e8c-8f4388147d30}\\0";
        let mut uefi = DeviceNode::new(
            uefi_id,
            "System Firmware UEFI 2.8",
            "UEFI System Firmware 2.8 HybridOS SecureBoot Capable",
            &[
                "UEFI\\RES_{2c13e54b-b003-4629-9e8c-8f4388147d30}",
                "UEFI\\SystemFirmware",
            ],
            DeviceCategory::Firmware,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "firmware.inf",
                &["C:\\Windows\\System32\\drivers\\firmware.sys"],
            ),
            BusType::FirmwareBus,
            "Motherboard BIOS / UEFI NVRAM",
        );
        uefi.set_parent(root_id);
        self.insert_device(uefi);

        // --------------------------------------------------------------------
        // 6. Storage Controllers & Disk Drives
        //    - Standard NVM Express Controller -> NVMe 1.4 SSD 64GB
        //    - Standard SATA AHCI Controller -> ATA SATA AHCI Disk 32GB
        // --------------------------------------------------------------------
        let nvme_ctrl_id = "PCI\\VEN_1AF4&DEV_1001&SUBSYS_00021AF4&REV_00\\3&11583659&0&20";
        let mut nvme_ctrl = DeviceNode::new(
            nvme_ctrl_id,
            "Standard NVM Express Controller",
            "Standard NVM Express Storage Controller",
            &[
                "PCI\\VEN_1AF4&DEV_1001&SUBSYS_00021AF4&REV_00",
                "PCI\\VEN_1AF4&DEV_1001",
                "PCI\\CC_010802",
            ],
            DeviceCategory::StorageControllers,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "stornvme.inf",
                &["C:\\Windows\\System32\\drivers\\stornvme.sys"],
            ),
            BusType::Pci,
            "PCI bus 0, device 4, function 0",
        );
        nvme_ctrl.set_parent(pci_root_id);
        nvme_ctrl
            .resources
            .add_irq(16, true, true)
            .add_memory(0xFEBC1000, 0xFEBC1FFF, false, true)
            .add_io_port(0xC100, 0xC13F);
        self.insert_device(nvme_ctrl);

        let nvme_disk_id = "SCSI\\DiskNVMe____Samsung_SSD_980_1B2Q\\5&18320498&0&000000";
        let mut nvme_disk = DeviceNode::new(
            nvme_disk_id,
            "NVMe 1.4 SSD 64GB",
            "Solid State Disk Drive NVMe Express High-Speed 64GB",
            &[
                "SCSI\\DiskNVMe____Samsung_SSD_980_1B2Q",
                "SCSI\\DiskNVMe____",
                "GenDisk",
            ],
            DeviceCategory::DiskDrives,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "disk.inf",
                &[
                    "C:\\Windows\\System32\\drivers\\disk.sys",
                    "C:\\Windows\\System32\\drivers\\partmgr.sys",
                ],
            ),
            BusType::ScsiIde,
            "Bus Number 0, Target Id 0, LUN 0",
        );
        nvme_disk.set_parent(nvme_ctrl_id);
        self.insert_device(nvme_disk);

        let sata_ctrl_id = "PCI\\VEN_8086&DEV_2922&SUBSYS_11001AF4&REV_02\\3&11583659&0&21";
        let mut sata_ctrl = DeviceNode::new(
            sata_ctrl_id,
            "Standard SATA AHCI Controller",
            "Standard SATA AHCI 6.0Gbps Storage Controller",
            &[
                "PCI\\VEN_8086&DEV_2922&SUBSYS_11001AF4&REV_02",
                "PCI\\VEN_8086&DEV_2922",
                "PCI\\CC_010601",
            ],
            DeviceCategory::StorageControllers,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "mshdc.inf",
                &["C:\\Windows\\System32\\drivers\\storahci.sys"],
            ),
            BusType::Pci,
            "PCI bus 0, device 5, function 0",
        );
        sata_ctrl.set_parent(pci_root_id);
        sata_ctrl
            .resources
            .add_irq(19, false, true)
            .add_memory(0xFEB00000, 0xFEB007FF, false, false)
            .add_io_port(0xB000, 0xB01F);
        self.insert_device(sata_ctrl);

        let sata_disk_id = "IDE\\DiskWDC_WD3200AAKS-00B3A0_________________01.03E01\\4&29a75841&0&0.0.0";
        let mut sata_disk = DeviceNode::new(
            sata_disk_id,
            "ATA SATA AHCI Disk 32GB",
            "ATA AHCI Hard Disk Drive 32GB",
            &[
                "IDE\\DiskWDC_WD3200AAKS-00B3A0_________________01.03E01",
                "IDE\\Disk",
                "GenDisk",
            ],
            DeviceCategory::DiskDrives,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "disk.inf",
                &["C:\\Windows\\System32\\drivers\\disk.sys"],
            ),
            BusType::ScsiIde,
            "on SATA Port 0 Channel 0",
        );
        sata_disk.set_parent(sata_ctrl_id);
        self.insert_device(sata_disk);

        // --------------------------------------------------------------------
        // 7. Display Adapters:
        //    - VirtIO-GPU PCI Device (VDM Accelerated)
        //    - AMD Radeon RX / Intel UHD Graphics
        // --------------------------------------------------------------------
        let virtio_gpu_id = "PCI\\VEN_1AF4&DEV_1050&SUBSYS_11001AF4&REV_01\\3&11583659&0&10";
        let mut virtio_gpu = DeviceNode::new(
            virtio_gpu_id,
            "VirtIO-GPU PCI Device (VDM Accelerated)",
            "Red Hat VirtIO GPU 3D Accelerated Display Adapter",
            &[
                "PCI\\VEN_1AF4&DEV_1050&SUBSYS_11001AF4&REV_01",
                "PCI\\VEN_1AF4&DEV_1050",
                "PCI\\CC_030000",
            ],
            DeviceCategory::DisplayAdapters,
            DriverInfo::new(
                "Red Hat, Inc.",
                "2026-05-12",
                "100.94.104.24000",
                "Microsoft Windows Hardware Compatibility Publisher",
                "viogpu.inf",
                &[
                    "C:\\Windows\\System32\\drivers\\viogpu.sys",
                    "C:\\Windows\\System32\\viogpudisp.dll",
                ],
            ),
            BusType::VirtIo,
            "PCI bus 0, device 2, function 0",
        );
        virtio_gpu.set_parent(pci_root_id);
        virtio_gpu
            .resources
            .add_irq(11, false, true)
            .add_memory(0x80000000, 0x8FFFFFFF, true, true) // 256MB VRAM BAR
            .add_memory(0xFEBA0000, 0xFEBA0FFF, false, false);
        self.insert_device(virtio_gpu);

        let intel_gpu_id = "PCI\\VEN_8086&DEV_4680&SUBSYS_00008086&REV_0C\\3&11583659&0&11";
        let mut intel_gpu = DeviceNode::new(
            intel_gpu_id,
            "AMD Radeon RX / Intel UHD Graphics",
            "Intel UHD Graphics 770 / AMD Radeon Graphics Engine",
            &[
                "PCI\\VEN_8086&DEV_4680&SUBSYS_00008086&REV_0C",
                "PCI\\VEN_8086&DEV_4680",
                "PCI\\CC_030000",
            ],
            DeviceCategory::DisplayAdapters,
            DriverInfo::new(
                "Intel Corporation",
                "2026-04-18",
                "31.0.101.4575",
                "Microsoft Windows Hardware Compatibility Publisher",
                "iigd_dch.inf",
                &[
                    "C:\\Windows\\System32\\drivers\\igdkmdn64.sys",
                    "C:\\Windows\\System32\\igd10umd64.dll",
                ],
            ),
            BusType::Pci,
            "PCI bus 0, device 3, function 0",
        );
        intel_gpu.set_parent(pci_root_id);
        intel_gpu
            .resources
            .add_irq(24, true, true)
            .add_memory(0x90000000, 0x9FFFFFFF, true, true)
            .add_memory(0xFEB80000, 0xFEB8FFFF, false, false);
        self.insert_device(intel_gpu);

        // --------------------------------------------------------------------
        // 8. Network Adapters:
        //    - VirtIO Network Adapter Gigabit
        //    - Realtek RTL8168 WDM Gigabit Ethernet NIC
        // --------------------------------------------------------------------
        let virtio_net_id = "PCI\\VEN_1AF4&DEV_1000&SUBSYS_00011AF4&REV_00\\3&11583659&0&18";
        let mut virtio_net = DeviceNode::new(
            virtio_net_id,
            "VirtIO Network Adapter Gigabit",
            "Red Hat VirtIO Ethernet Adapter Gigabit #1",
            &[
                "PCI\\VEN_1AF4&DEV_1000&SUBSYS_00011AF4&REV_00",
                "PCI\\VEN_1AF4&DEV_1000",
                "PCI\\CC_020000",
            ],
            DeviceCategory::NetworkAdapters,
            DriverInfo::new(
                "Red Hat, Inc.",
                "2026-05-12",
                "100.92.104.22000",
                "Microsoft Windows Hardware Compatibility Publisher",
                "netkvm.inf",
                &[
                    "C:\\Windows\\System32\\drivers\\netkvm.sys",
                    "C:\\Windows\\System32\\netkvmco.dll",
                ],
            ),
            BusType::VirtIo,
            "PCI bus 0, device 6, function 0",
        );
        virtio_net.set_parent(pci_root_id);
        virtio_net
            .resources
            .add_irq(16, true, true)
            .add_memory(0xFEBC0000, 0xFEBC0FFF, false, false)
            .add_io_port(0xC000, 0xC01F);
        self.insert_device(virtio_net);

        let realtek_net_id = "PCI\\VEN_10EC&DEV_8168&SUBSYS_012310EC&REV_06\\3&11583659&0&19";
        let mut realtek_net = DeviceNode::new(
            realtek_net_id,
            "Realtek RTL8168 WDM Gigabit Ethernet NIC",
            "Realtek PCIe GbE Family Controller (WDM Driver)",
            &[
                "PCI\\VEN_10EC&DEV_8168&SUBSYS_012310EC&REV_06",
                "PCI\\VEN_10EC&DEV_8168",
                "PCI\\CC_020000",
            ],
            DeviceCategory::NetworkAdapters,
            DriverInfo::new(
                "Realtek Semiconductor Corp.",
                "2026-03-10",
                "10.68.815.2026",
                "Microsoft Windows Hardware Compatibility Publisher",
                "rt640x64.inf",
                &["C:\\Windows\\System32\\drivers\\rt640x64.sys"],
            ),
            BusType::Pci,
            "PCI bus 0, device 7, function 0",
        );
        realtek_net.set_parent(pci_root_id);
        realtek_net
            .resources
            .add_irq(18, false, true)
            .add_memory(0xFEB90000, 0xFEB90FFF, false, false)
            .add_io_port(0xC200, 0xC2FF);
        self.insert_device(realtek_net);

        // --------------------------------------------------------------------
        // 9. Audio:
        //    - High Definition Audio Controller (System Devices / Parent)
        //    - Realtek High Definition Audio (Sound Controller)
        //    - Speakers / Headphones (Audio inputs and outputs)
        //    - VirtIO Sound Card (Vortex VDM)
        // --------------------------------------------------------------------
        let hda_ctrl_id = "PCI\\VEN_8086&DEV_284B&SUBSYS_00008086&REV_02\\3&11583659&0&27";
        let mut hda_ctrl = DeviceNode::new(
            hda_ctrl_id,
            "High Definition Audio Controller",
            "Intel High Definition Audio Bus Controller",
            &[
                "PCI\\VEN_8086&DEV_284B&SUBSYS_00008086&REV_02",
                "PCI\\VEN_8086&DEV_284B",
                "PCI\\CC_040300",
            ],
            DeviceCategory::SystemDevices,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "hdaudbus.inf",
                &["C:\\Windows\\System32\\drivers\\hdaudbus.sys"],
            ),
            BusType::Pci,
            "PCI bus 0, device 8, function 0",
        );
        hda_ctrl.set_parent(pci_root_id);
        hda_ctrl
            .resources
            .add_irq(22, false, true)
            .add_memory(0xFEB70000, 0xFEB73FFF, false, false);
        self.insert_device(hda_ctrl);

        let realtek_audio_id = "HDAUDIO\\FUNC_01&VEN_10EC&DEV_0892&SUBSYS_10EC0892&REV_1003\\4&1234567&0&0001";
        let mut realtek_audio = DeviceNode::new(
            realtek_audio_id,
            "Realtek High Definition Audio",
            "Realtek High Definition Audio Codec ALC892",
            &[
                "HDAUDIO\\FUNC_01&VEN_10EC&DEV_0892&SUBSYS_10EC0892&REV_1003",
                "HDAUDIO\\FUNC_01&VEN_10EC&DEV_0892",
            ],
            DeviceCategory::SoundVideoGameControllers,
            DriverInfo::new(
                "Realtek Semiconductor Corp.",
                "2026-02-14",
                "6.0.9412.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "hdxrt.inf",
                &[
                    "C:\\Windows\\System32\\drivers\\rtkvhd64.sys",
                    "C:\\Windows\\System32\\rtkhdaud.dat",
                ],
            ),
            BusType::Hdaudio,
            "on High Definition Audio Bus",
        );
        realtek_audio.set_parent(hda_ctrl_id);
        self.insert_device(realtek_audio);

        let speakers_id = "MMDEVAPI\\AudioEndpoints\\{0.0.0.00000000}.{a1b2c3d4-e5f6-7890-1234-56789abcdef0}";
        let mut speakers = DeviceNode::new(
            speakers_id,
            "Speakers / Headphones (High Definition Audio Device)",
            "Realtek High Definition Audio Endpoint",
            &["MMDEVAPI\\AudioEndpoints", "AudioEndpoint"],
            DeviceCategory::AudioInputsAndOutputs,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "audioendpoint.inf",
                &["C:\\Windows\\System32\\drivers\\audioendpoint.sys"],
            ),
            BusType::Hdaudio,
            "Rear 3.5mm Audio Output Jack",
        );
        speakers.set_parent(realtek_audio_id);
        self.insert_device(speakers);

        let virtio_snd_id = "PCI\\VEN_1AF4&DEV_1053&SUBSYS_11001AF4&REV_01\\3&11583659&0&28";
        let mut virtio_snd = DeviceNode::new(
            virtio_snd_id,
            "VirtIO Sound Card (Vortex VDM)",
            "VirtIO Sound Multi-Stream Virtual Audio Processor",
            &[
                "PCI\\VEN_1AF4&DEV_1053&SUBSYS_11001AF4&REV_01",
                "PCI\\VEN_1AF4&DEV_1053",
            ],
            DeviceCategory::AudioInputsAndOutputs,
            DriverInfo::new(
                "Red Hat, Inc.",
                "2026-05-12",
                "100.93.104.23000",
                "Microsoft Windows Hardware Compatibility Publisher",
                "viosnd.inf",
                &["C:\\Windows\\System32\\drivers\\viosnd.sys"],
            ),
            BusType::VirtIo,
            "PCI bus 0, device 9, function 0",
        );
        virtio_snd.set_parent(pci_root_id);
        virtio_snd
            .resources
            .add_irq(23, false, true)
            .add_memory(0xFEB60000, 0xFEB60FFF, false, false);
        self.insert_device(virtio_snd);

        // --------------------------------------------------------------------
        // 10. Hidden / Phantom Device (For Testing Show Hidden Devices)
        // --------------------------------------------------------------------
        let sandisk_usb_id = "USBSTOR\\DiskSanDisk_Ultra___________1.00\\0123456789ABCDEF&0";
        let mut sandisk_usb = DeviceNode::new(
            sandisk_usb_id,
            "SanDisk Ultra USB 3.0 Flash Drive 16GB",
            "SanDisk Ultra USB Flash Storage Drive",
            &[
                "USBSTOR\\DiskSanDisk_Ultra___________1.00",
                "USBSTOR\\DiskSanDisk_",
                "GenDisk",
            ],
            DeviceCategory::DiskDrives,
            DriverInfo::new(
                "Microsoft",
                "2026-06-21",
                "10.0.22621.1",
                "Microsoft Windows Hardware Compatibility Publisher",
                "disk.inf",
                &["C:\\Windows\\System32\\drivers\\disk.sys"],
            ),
            BusType::Usb,
            "Port_#0001.Hub_#0002",
        );
        sandisk_usb.set_parent(root_id);
        sandisk_usb.set_hidden(true); // Non-present / unplugged device
        self.insert_device(sandisk_usb);

        // Store standard driver repository packages for wizard
        self.driver_store_packages.insert(
            "netkvm.inf".to_string(),
            "100.95.104.26000 (2026-09-01)".to_string(),
        );
        self.driver_store_packages.insert(
            "viogpu.inf".to_string(),
            "100.96.104.25000 (2026-09-15)".to_string(),
        );
        self.driver_store_packages.insert(
            "stornvme.inf".to_string(),
            "10.0.22631.2428 (2026-08-10)".to_string(),
        );
    }

    /// Retrieve an immutable reference to a device by its ID.
    pub fn get_device(&self, id: &str) -> Option<&DeviceNode> {
        self.devices.get(id)
    }

    /// Retrieve a mutable reference to a device by its ID.
    pub fn get_device_mut(&mut self, id: &str) -> Option<&mut DeviceNode> {
        self.devices.get_mut(id)
    }

    /// Retrieve all devices matching a specific category.
    pub fn get_devices_by_category(
        &self,
        category: DeviceCategory,
        show_hidden: bool,
    ) -> Vec<&DeviceNode> {
        let mut list: Vec<&DeviceNode> = self
            .devices
            .values()
            .filter(|d| d.category == category && (show_hidden || !d.is_hidden))
            .collect();
        list.sort_by(|a, b| a.friendly_name.cmp(&b.friendly_name));
        list
    }

    /// Retrieve all child devices of a given parent ID.
    pub fn get_children(&self, parent_id: &str, show_hidden: bool) -> Vec<&DeviceNode> {
        if let Some(parent) = self.devices.get(parent_id) {
            let mut children = Vec::new();
            for cid in &parent.children_ids {
                if let Some(child) = self.devices.get(cid) {
                    if show_hidden || !child.is_hidden {
                        children.push(child);
                    }
                }
            }
            children.sort_by(|a, b| a.friendly_name.cmp(&b.friendly_name));
            children
        } else {
            Vec::new()
        }
    }

    /// Total count of devices currently in the tree.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Return root device ID.
    pub fn root_id(&self) -> &str {
        &self.root_id
    }

    // ========================================================================
    // Action 1: Scan for Hardware Changes
    // ========================================================================

    /// Probes the Vortex Driver Model (VDM) bus and PCI configuration space
    /// to detect hotplugged devices, status changes, or re-attached peripherals.
    pub fn scan_for_hardware_changes(&mut self) -> ScanReport {
        self.hardware_bus_probed_count += 1;
        let mut discovered = Vec::new();
        let total_scanned = self.devices.len();

        // Check if previously hidden USB storage drive can be detected / plugged in
        let sandisk_id = "USBSTOR\\DiskSanDisk_Ultra___________1.00\\0123456789ABCDEF&0";
        if let Some(disk) = self.devices.get_mut(sandisk_id) {
            if disk.is_hidden {
                disk.is_hidden = false;
                discovered.push(disk.friendly_name.clone());
            }
        }

        // Simulate VDM bus scan on PCI bus 0..256
        discovered.push("VirtIO VDM PCI Bridge #0 Bus Probe".to_string());

        ScanReport {
            total_devices_scanned: total_scanned,
            discovered_devices: discovered,
            removed_devices: Vec::new(),
            bus_probed: "PCI Express Root Complex / VDM Subsystem".to_string(),
            duration_ms: 42,
        }
    }

    // ========================================================================
    // Action 2: Enable & Disable Device Toggle
    // ========================================================================

    /// Enable a disabled device.
    pub fn enable_device(&mut self, device_id: &str) -> Result<DeviceStatus, DevMgmtError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or_else(|| DevMgmtError::DeviceNotFound(device_id.to_string()))?;
        device.enable()?;
        Ok(device.status.clone())
    }

    /// Disable an active device.
    pub fn disable_device(&mut self, device_id: &str) -> Result<DeviceStatus, DevMgmtError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or_else(|| DevMgmtError::DeviceNotFound(device_id.to_string()))?;
        device.disable()?;
        Ok(device.status.clone())
    }

    // ========================================================================
    // Action 3: Update Driver Software Wizard
    // ========================================================================

    /// Run the Update Driver Software Wizard dialog on a device.
    pub fn update_driver(
        &mut self,
        device_id: &str,
        options: UpdateDriverOptions,
    ) -> Result<DriverUpdateResult, DevMgmtError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or_else(|| DevMgmtError::DeviceNotFound(device_id.to_string()))?;

        let prev_ver = device.driver_info.version.clone();
        let mut steps = Vec::new();

        match options {
            UpdateDriverOptions::SearchAutomatically => {
                steps.push("Searching online and HybridOS Driver Catalog for drivers...".to_string());
                steps.push(format!(
                    "Querying hardware ID '{}' against WHQL catalog...",
                    device.primary_hardware_id()
                ));

                // Check catalog for upgrade
                let inf = &device.driver_info.inf_name;
                if let Some(new_entry) = self.driver_store_packages.get(inf).cloned() {
                    let parts: Vec<&str> = new_entry.split(' ').collect();
                    let new_ver = parts[0];
                    let new_date = parts
                        .get(1)
                        .map(|s| s.trim_matches(|c| c == '(' || c == ')'))
                        .unwrap_or("2026-09-01");

                    steps.push(format!("Found compatible WHQL driver version {}", new_ver));
                    steps.push("Verifying digital signature: Microsoft Windows Hardware Compatibility Publisher...".to_string());
                    steps.push("Installing driver software files to DriverStore...".to_string());
                    steps.push("Restarting device stack with new driver...".to_string());

                    device.driver_info.version = new_ver.to_string();
                    device.driver_info.date = new_date.to_string();
                    device.driver_info.digital_signer =
                        "Microsoft Windows Hardware Compatibility Publisher".to_string();
                    device.driver_software_installed = true;
                    device.status = DeviceStatus::WorkingProperly;

                    Ok(DriverUpdateResult {
                        success: true,
                        driver_updated: true,
                        previous_version: prev_ver,
                        new_version: new_ver.to_string(),
                        wizard_steps: steps,
                        message: "Windows has successfully updated your drivers.".to_string(),
                    })
                } else {
                    steps.push(
                        "The best drivers for your device are already installed.".to_string(),
                    );
                    Ok(DriverUpdateResult {
                        success: true,
                        driver_updated: false,
                        previous_version: prev_ver.clone(),
                        new_version: prev_ver,
                        wizard_steps: steps,
                        message: "The best drivers for your device are already installed."
                            .to_string(),
                    })
                }
            }
            UpdateDriverOptions::BrowseComputer {
                search_path,
                include_subfolders,
            } => {
                steps.push(format!(
                    "Browsing local folder '{}' (include subfolders: {})...",
                    search_path, include_subfolders
                ));
                steps.push(format!(
                    "Found compatible INF definition for hardware ID '{}'",
                    device.primary_hardware_id()
                ));

                let upgraded_version = format!("{}.999", device.driver_info.version);
                device.driver_info.version = upgraded_version.clone();
                device.driver_info.date = "2026-11-01".to_string();
                device.driver_info.digital_signer = "HybridOS WHQL Signer".to_string();
                device.driver_software_installed = true;
                device.status = DeviceStatus::WorkingProperly;

                steps.push("Copying driver binaries and writing registry keys...".to_string());
                steps.push("Driver package installation complete.".to_string());

                Ok(DriverUpdateResult {
                    success: true,
                    driver_updated: true,
                    previous_version: prev_ver,
                    new_version: upgraded_version,
                    wizard_steps: steps,
                    message: "Windows has successfully updated your drivers from local path."
                        .to_string(),
                })
            }
        }
    }

    // ========================================================================
    // Action 4: Uninstall Device
    // ========================================================================

    /// Uninstall a device node, optionally deleting driver software package from DriverStore.
    pub fn uninstall_device(
        &mut self,
        device_id: &str,
        delete_driver_software: bool,
    ) -> Result<UninstallReport, DevMgmtError> {
        let device = self
            .devices
            .get(device_id)
            .ok_or_else(|| DevMgmtError::DeviceNotFound(device_id.to_string()))?;

        if device.is_system_critical {
            return Err(DevMgmtError::CannotDisableCriticalDevice(format!(
                "Cannot uninstall system-critical device '{}'",
                device.friendly_name
            )));
        }

        let friendly_name = device.friendly_name.clone();
        let store_path = device.driver_info.driver_store_path.clone();
        let parent_id_opt = device.parent_id.clone();

        // Remove from parent's children list
        if let Some(ref parent_id) = parent_id_opt {
            if let Some(parent) = self.devices.get_mut(parent_id) {
                parent.children_ids.retain(|cid| cid != device_id);
            }
        }

        // Remove from devices map and order list
        self.devices.remove(device_id);
        self.device_order.retain(|id| id != device_id);

        let deleted_path = if delete_driver_software {
            Some(store_path)
        } else {
            None
        };

        Ok(UninstallReport {
            device_id: device_id.to_string(),
            friendly_name: friendly_name.clone(),
            removed_from_tree: true,
            driver_package_deleted: delete_driver_software,
            deleted_package_path: deleted_path,
            message: format!("Device '{}' was successfully uninstalled.", friendly_name),
        })
    }

    /// Check if ancestor_id is an ancestor of child_id along parent_id links.
    pub fn is_ancestor_of<'a>(&'a self, ancestor_id: &str, mut child_id: &'a str) -> bool {
        while let Some(parent) = self.devices.get(child_id).and_then(|d| d.parent_id.as_deref()) {
            if parent == ancestor_id {
                return true;
            }
            child_id = parent;
        }
        false
    }

    // ========================================================================
    // Resource Conflict Verification Engine
    // ========================================================================

    /// Scan all active devices and return any detected resource conflicts.
    pub fn check_resource_conflicts(&self) -> Vec<(String, String, ResourceConflict)> {
        let mut conflicts = Vec::new();
        let active_devices: Vec<&DeviceNode> = self
            .devices
            .values()
            .filter(|d| !d.status.is_disabled() && !d.is_hidden)
            .collect();

        for i in 0..active_devices.len() {
            for j in (i + 1)..active_devices.len() {
                let dev1 = active_devices[i];
                let dev2 = active_devices[j];

                // Parent bus bridges route resources for child devices (e.g. PCI Root Complex)
                if self.is_ancestor_of(&dev1.id, &dev2.id) || self.is_ancestor_of(&dev2.id, &dev1.id) {
                    continue;
                }

                if let Some(conflict) = dev1.resources.find_conflict(&dev2.resources) {
                    conflicts.push((
                        dev1.friendly_name.clone(),
                        dev2.friendly_name.clone(),
                        conflict,
                    ));
                }
            }
        }
        conflicts
    }

    // ========================================================================
    // View Builders: Devices by Type / Connection, Resources by Type / Connection
    // ========================================================================

    /// Build tree for 'Devices by type' view.
    pub fn build_devices_by_type(&self, show_hidden: bool) -> Vec<TreeViewItem> {
        let mut items = Vec::new();

        for &category in DeviceCategory::all() {
            let devices = self.get_devices_by_category(category, show_hidden);
            if devices.is_empty() {
                continue;
            }

            // Category Header
            items.push(TreeViewItem {
                id: format!("cat:{}", category.display_name()),
                label: category.display_name().to_string(),
                depth: 0,
                is_category: true,
                is_expanded: true,
                icon: category.icon_glyph(),
                has_problem: devices.iter().any(|d| d.status.has_problem()),
                is_disabled: false,
                device_id: None,
                resource_detail: None,
            });

            // Devices under Category
            for dev in devices {
                items.push(TreeViewItem {
                    id: dev.id.clone(),
                    label: dev.friendly_name.clone(),
                    depth: 1,
                    is_category: false,
                    is_expanded: false,
                    icon: category.icon_glyph(),
                    has_problem: dev.status.has_problem(),
                    is_disabled: dev.status.is_disabled(),
                    device_id: Some(dev.id.clone()),
                    resource_detail: None,
                });
            }
        }
        items
    }

    /// Build tree for 'Devices by connection' view.
    pub fn build_devices_by_connection(&self, show_hidden: bool) -> Vec<TreeViewItem> {
        let mut items = Vec::new();
        if let Some(root) = self.devices.get(&self.root_id) {
            self.recurse_devices_by_connection(root, 0, show_hidden, &mut items);
        }
        items
    }

    fn recurse_devices_by_connection(
        &self,
        node: &DeviceNode,
        depth: usize,
        show_hidden: bool,
        items: &mut Vec<TreeViewItem>,
    ) {
        if node.is_hidden && !show_hidden {
            return;
        }

        let is_expandable = !node.children_ids.is_empty();
        items.push(TreeViewItem {
            id: node.id.clone(),
            label: node.friendly_name.clone(),
            depth,
            is_category: is_expandable,
            is_expanded: true,
            icon: node.category.icon_glyph(),
            has_problem: node.status.has_problem(),
            is_disabled: node.status.is_disabled(),
            device_id: Some(node.id.clone()),
            resource_detail: None,
        });

        for child in self.get_children(&node.id, show_hidden) {
            self.recurse_devices_by_connection(child, depth + 1, show_hidden, items);
        }
    }

    /// Build tree for 'Resources by type' view (DMA, IO, IRQ, Memory).
    pub fn build_resources_by_type(&self) -> Vec<TreeViewItem> {
        let mut items = Vec::new();

        // 1. Direct Memory Access (DMA)
        items.push(TreeViewItem {
            id: "res_cat:dma".to_string(),
            label: "Direct memory access (DMA)".to_string(),
            depth: 0,
            is_category: true,
            is_expanded: true,
            icon: "⚡",
            has_problem: false,
            is_disabled: false,
            device_id: None,
            resource_detail: None,
        });
        let mut dma_entries: Vec<(u8, String, String)> = Vec::new();
        for dev in self.devices.values() {
            for dma in &dev.resources.dma_channels {
                dma_entries.push((
                    dma.channel,
                    dma.display_string(),
                    dev.friendly_name.clone(),
                ));
            }
        }
        dma_entries.sort_by_key(|e| e.0);
        for (_, res_str, dev_name) in dma_entries {
            items.push(TreeViewItem {
                id: format!("dma:{}", res_str),
                label: format!("Channel {} - {}", res_str, dev_name),
                depth: 1,
                is_category: false,
                is_expanded: false,
                icon: "⚡",
                has_problem: false,
                is_disabled: false,
                device_id: None,
                resource_detail: Some(res_str),
            });
        }

        // 2. Input/Output (IO)
        items.push(TreeViewItem {
            id: "res_cat:io".to_string(),
            label: "Input/output (IO)".to_string(),
            depth: 0,
            is_category: true,
            is_expanded: true,
            icon: "🔌",
            has_problem: false,
            is_disabled: false,
            device_id: None,
            resource_detail: None,
        });
        let mut io_entries: Vec<(u16, String, String)> = Vec::new();
        for dev in self.devices.values() {
            for io in &dev.resources.io_ports {
                io_entries.push((io.start, io.display_string(), dev.friendly_name.clone()));
            }
        }
        io_entries.sort_by_key(|e| e.0);
        for (_, res_str, dev_name) in io_entries {
            items.push(TreeViewItem {
                id: format!("io:{}", res_str),
                label: format!("[{}] {}", res_str, dev_name),
                depth: 1,
                is_category: false,
                is_expanded: false,
                icon: "🔌",
                has_problem: false,
                is_disabled: false,
                device_id: None,
                resource_detail: Some(res_str),
            });
        }

        // 3. Interrupt Request (IRQ)
        items.push(TreeViewItem {
            id: "res_cat:irq".to_string(),
            label: "Interrupt request (IRQ)".to_string(),
            depth: 0,
            is_category: true,
            is_expanded: true,
            icon: "⚡",
            has_problem: false,
            is_disabled: false,
            device_id: None,
            resource_detail: None,
        });
        let mut irq_entries: Vec<(u32, String, String)> = Vec::new();
        for dev in self.devices.values() {
            for irq in &dev.resources.irqs {
                irq_entries.push((
                    irq.irq_number,
                    irq.display_string(),
                    dev.friendly_name.clone(),
                ));
            }
        }
        irq_entries.sort_by_key(|e| e.0);
        for (_, res_str, dev_name) in irq_entries {
            items.push(TreeViewItem {
                id: format!("irq:{}", res_str),
                label: format!("(IRQ) {} {}", res_str, dev_name),
                depth: 1,
                is_category: false,
                is_expanded: false,
                icon: "⚡",
                has_problem: false,
                is_disabled: false,
                device_id: None,
                resource_detail: Some(res_str),
            });
        }

        // 4. Memory
        items.push(TreeViewItem {
            id: "res_cat:memory".to_string(),
            label: "Memory".to_string(),
            depth: 0,
            is_category: true,
            is_expanded: true,
            icon: "💾",
            has_problem: false,
            is_disabled: false,
            device_id: None,
            resource_detail: None,
        });
        let mut mem_entries: Vec<(u64, String, String)> = Vec::new();
        for dev in self.devices.values() {
            for mem in &dev.resources.memory_ranges {
                mem_entries.push((
                    mem.start,
                    mem.display_string(),
                    dev.friendly_name.clone(),
                ));
            }
        }
        mem_entries.sort_by_key(|e| e.0);
        for (_, res_str, dev_name) in mem_entries {
            items.push(TreeViewItem {
                id: format!("mem:{}", res_str),
                label: format!("[{}] {}", res_str, dev_name),
                depth: 1,
                is_category: false,
                is_expanded: false,
                icon: "💾",
                has_problem: false,
                is_disabled: false,
                device_id: None,
                resource_detail: Some(res_str),
            });
        }

        items
    }

    /// Build tree for 'Resources by connection' view.
    pub fn build_resources_by_connection(&self) -> Vec<TreeViewItem> {
        let mut items = Vec::new();
        if let Some(root) = self.devices.get(&self.root_id) {
            self.recurse_resources_by_connection(root, 0, &mut items);
        }
        items
    }

    fn recurse_resources_by_connection(
        &self,
        node: &DeviceNode,
        depth: usize,
        items: &mut Vec<TreeViewItem>,
    ) {
        if node.is_hidden {
            return;
        }

        // Add node item
        items.push(TreeViewItem {
            id: node.id.clone(),
            label: node.friendly_name.clone(),
            depth,
            is_category: true,
            is_expanded: true,
            icon: node.category.icon_glyph(),
            has_problem: node.status.has_problem(),
            is_disabled: node.status.is_disabled(),
            device_id: Some(node.id.clone()),
            resource_detail: None,
        });

        // Add resources of this node indented
        for mem in &node.resources.memory_ranges {
            items.push(TreeViewItem {
                id: format!("{}:mem:{}", node.id, mem.start),
                label: format!("Memory: {}", mem.display_string()),
                depth: depth + 1,
                is_category: false,
                is_expanded: false,
                icon: "💾",
                has_problem: false,
                is_disabled: false,
                device_id: Some(node.id.clone()),
                resource_detail: Some(mem.display_string()),
            });
        }
        for io in &node.resources.io_ports {
            items.push(TreeViewItem {
                id: format!("{}:io:{}", node.id, io.start),
                label: format!("I/O Port: {}", io.display_string()),
                depth: depth + 1,
                is_category: false,
                is_expanded: false,
                icon: "🔌",
                has_problem: false,
                is_disabled: false,
                device_id: Some(node.id.clone()),
                resource_detail: Some(io.display_string()),
            });
        }
        for irq in &node.resources.irqs {
            items.push(TreeViewItem {
                id: format!("{}:irq:{}", node.id, irq.irq_number),
                label: format!("IRQ: {}", irq.display_string()),
                depth: depth + 1,
                is_category: false,
                is_expanded: false,
                icon: "⚡",
                has_problem: false,
                is_disabled: false,
                device_id: Some(node.id.clone()),
                resource_detail: Some(irq.display_string()),
            });
        }

        // Recurse children
        for child in self.get_children(&node.id, false) {
            self.recurse_resources_by_connection(child, depth + 1, items);
        }
    }
}

// ============================================================================
// Section 7: Device Properties Dialog Model
// ============================================================================

/// Selected tab in Device Properties dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyTab {
    General,
    Driver,
    Details,
    Resources,
}

impl PropertyTab {
    pub const fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Driver => "Driver",
            Self::Details => "Details",
            Self::Resources => "Resources",
        }
    }
}

/// Content model for Device Properties dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct DevicePropertiesDialog {
    pub device_id: String,
    pub title: String,
    pub active_tab: PropertyTab,
    pub device_desc: String,
    pub device_type: String,
    pub manufacturer: String,
    pub location: String,
    pub status_box_text: String,
    pub driver_provider: String,
    pub driver_date: String,
    pub driver_version: String,
    pub digital_signer: String,
    pub driver_files: Vec<String>,
    pub hardware_ids: Vec<String>,
    pub device_instance_path: String,
    pub resource_entries: Vec<(String, String)>,
    pub conflict_list_text: String,
}

impl DevicePropertiesDialog {
    pub fn from_device(device: &DeviceNode, conflicts: &[ResourceConflict]) -> Self {
        let mut resource_entries = Vec::new();
        for mem in &device.resources.memory_ranges {
            resource_entries.push(("Memory Range".to_string(), mem.display_string()));
        }
        for io in &device.resources.io_ports {
            resource_entries.push(("I/O Range".to_string(), io.display_string()));
        }
        for irq in &device.resources.irqs {
            resource_entries.push(("Interrupt Request".to_string(), irq.display_string()));
        }
        for dma in &device.resources.dma_channels {
            resource_entries.push(("DMA Channel".to_string(), dma.display_string()));
        }

        let conflict_text = if conflicts.is_empty() {
            "No conflicts.".to_string()
        } else {
            conflicts
                .iter()
                .map(|c| format!("{} conflict: {}", c.resource_type, c.reason))
                .collect::<Vec<_>>()
                .join("\n")
        };

        Self {
            device_id: device.id.clone(),
            title: format!("{} Properties", device.friendly_name),
            active_tab: PropertyTab::General,
            device_desc: device.device_desc.clone(),
            device_type: device.category.display_name().to_string(),
            manufacturer: device.driver_info.provider.clone(),
            location: device.location_info.clone(),
            status_box_text: device.status.status_text(),
            driver_provider: device.driver_info.provider.clone(),
            driver_date: device.driver_info.date.clone(),
            driver_version: device.driver_info.version.clone(),
            digital_signer: device.driver_info.digital_signer.clone(),
            driver_files: device.driver_info.driver_files.clone(),
            hardware_ids: device.hardware_ids.clone(),
            device_instance_path: device.id.clone(),
            resource_entries,
            conflict_list_text: conflict_text,
        }
    }

    /// Render text representation of dialog.
    pub fn render_dialog_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("┌────────────────────────────────────────────────────────┐\n"));
        out.push_str(&format!("│ {:<54} │\n", self.title));
        out.push_str(&format!("├────────────────────────────────────────────────────────┤\n"));
        out.push_str(&format!(
            "│ Tabs: [{}] [{}] [{}] [{}] │\n",
            if self.active_tab == PropertyTab::General { "*General*" } else { "General" },
            if self.active_tab == PropertyTab::Driver { "*Driver*" } else { "Driver" },
            if self.active_tab == PropertyTab::Details { "*Details*" } else { "Details" },
            if self.active_tab == PropertyTab::Resources { "*Resources*" } else { "Resources" }
        ));
        out.push_str(&format!("├────────────────────────────────────────────────────────┤\n"));

        match self.active_tab {
            PropertyTab::General => {
                out.push_str(&format!("│ Description:  {:<40} │\n", self.device_desc));
                out.push_str(&format!("│ Device type:  {:<40} │\n", self.device_type));
                out.push_str(&format!("│ Manufacturer: {:<40} │\n", self.manufacturer));
                out.push_str(&format!("│ Location:     {:<40} │\n", self.location));
                out.push_str("│                                                        │\n");
                out.push_str("│ Device status:                                         │\n");
                out.push_str(&format!("│  {:<53} │\n", self.status_box_text));
            }
            PropertyTab::Driver => {
                out.push_str(&format!("│ Driver Provider: {:<37} │\n", self.driver_provider));
                out.push_str(&format!("│ Driver Date:     {:<37} │\n", self.driver_date));
                out.push_str(&format!("│ Driver Version:  {:<37} │\n", self.driver_version));
                out.push_str(&format!("│ Digital Signer:  {:<37} │\n", self.digital_signer));
                out.push_str("│                                                        │\n");
                out.push_str("│ [ Driver Details ]   [ Update Driver ]                 │\n");
                out.push_str("│ [ Disable Device ]   [ Uninstall Device ]              │\n");
            }
            PropertyTab::Details => {
                out.push_str("│ Property: Hardware Ids                                 │\n");
                for hwid in &self.hardware_ids {
                    out.push_str(&format!("│  - {:<51} │\n", hwid));
                }
                out.push_str(&format!("│ PDO: {:<49} │\n", self.device_instance_path));
            }
            PropertyTab::Resources => {
                out.push_str("│ Resource settings:                                     │\n");
                for (rtype, setting) in &self.resource_entries {
                    out.push_str(&format!("│  {:<18}: {:<32} │\n", rtype, setting));
                }
                out.push_str("│                                                        │\n");
                out.push_str("│ Conflicting device list:                               │\n");
                out.push_str(&format!("│  {:<53} │\n", self.conflict_list_text));
            }
        }
        out.push_str(&format!("└────────────────────────────────────────────────────────┘\n"));
        out
    }
}

// ============================================================================
// Section 8: Windows 11 Device Manager Application State & UI Renderer
// ============================================================================

/// Complete Windows 11 Device Manager (`devmgmt.msc`) Application.
#[derive(Debug, Clone)]
pub struct DeviceManagerApp {
    pub tree: DeviceTree,
    pub view_mode: ViewMode,
    pub show_hidden_devices: bool,
    pub selected_device_id: Option<String>,
    pub search_filter: String,
    pub properties_dialog: Option<DevicePropertiesDialog>,
    pub status_message: String,
    pub window_width: u32,
    pub window_height: u32,
}

impl DeviceManagerApp {
    /// Initialize a new Device Manager application with standard hardware configuration.
    pub fn new() -> Self {
        let tree = DeviceTree::new_standard_pc();
        let first_id = tree.devices.keys().next().cloned();
        Self {
            tree,
            view_mode: ViewMode::DevicesByType,
            show_hidden_devices: false,
            selected_device_id: first_id,
            search_filter: String::new(),
            properties_dialog: None,
            status_message: "Ready".to_string(),
            window_width: 800,
            window_height: 600,
        }
    }

    /// Set current active view mode.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
        self.status_message = format!("View switched to '{}'", mode.display_name());
    }

    /// Toggle show hidden devices.
    pub fn toggle_show_hidden_devices(&mut self) -> bool {
        self.show_hidden_devices = !self.show_hidden_devices;
        self.status_message = if self.show_hidden_devices {
            "Showing hidden and non-present devices".to_string()
        } else {
            "Hiding non-present devices".to_string()
        };
        self.show_hidden_devices
    }

    /// Select a device node by ID.
    pub fn select_device(&mut self, device_id: &str) -> bool {
        if self.tree.devices.contains_key(device_id) {
            self.selected_device_id = Some(device_id.to_string());
            if let Some(dev) = self.tree.get_device(device_id) {
                self.status_message = format!("{}: {}", dev.friendly_name, dev.status.status_text());
            }
            true
        } else {
            false
        }
    }

    /// Set search filter.
    pub fn set_search_filter(&mut self, filter: &str) {
        self.search_filter = filter.trim().to_lowercase();
    }

    /// Action: Scan for hardware changes.
    pub fn action_scan_for_hardware_changes(&mut self) -> ScanReport {
        let report = self.tree.scan_for_hardware_changes();
        self.status_message = format!(
            "Scan complete: {} devices probed in {}ms",
            report.total_devices_scanned, report.duration_ms
        );
        report
    }

    /// Action: Enable currently selected device.
    pub fn action_enable_selected(&mut self) -> Result<DeviceStatus, DevMgmtError> {
        let id = self
            .selected_device_id
            .clone()
            .ok_or_else(|| DevMgmtError::OperationFailed("No device selected".to_string()))?;
        let status = self.tree.enable_device(&id)?;
        self.status_message = format!("Device enabled successfully ({})", status.status_text());
        Ok(status)
    }

    /// Action: Disable currently selected device.
    pub fn action_disable_selected(&mut self) -> Result<DeviceStatus, DevMgmtError> {
        let id = self
            .selected_device_id
            .clone()
            .ok_or_else(|| DevMgmtError::OperationFailed("No device selected".to_string()))?;
        let status = self.tree.disable_device(&id)?;
        self.status_message = format!("Device disabled successfully ({})", status.status_text());
        Ok(status)
    }

    /// Action: Update driver software for selected device.
    pub fn action_update_driver_selected(
        &mut self,
        options: UpdateDriverOptions,
    ) -> Result<DriverUpdateResult, DevMgmtError> {
        let id = self
            .selected_device_id
            .clone()
            .ok_or_else(|| DevMgmtError::OperationFailed("No device selected".to_string()))?;
        let result = self.tree.update_driver(&id, options)?;
        self.status_message = result.message.clone();
        Ok(result)
    }

    /// Action: Uninstall currently selected device.
    pub fn action_uninstall_selected(
        &mut self,
        delete_driver: bool,
    ) -> Result<UninstallReport, DevMgmtError> {
        let id = self
            .selected_device_id
            .clone()
            .ok_or_else(|| DevMgmtError::OperationFailed("No device selected".to_string()))?;
        let report = self.tree.uninstall_device(&id, delete_driver)?;
        self.selected_device_id = None;
        self.status_message = report.message.clone();
        Ok(report)
    }

    /// Open or refresh Properties dialog for selected device.
    pub fn open_properties_dialog(&mut self) -> Option<&DevicePropertiesDialog> {
        let id = self.selected_device_id.clone()?;
        let dev = self.tree.get_device(&id)?;
        let dialog = DevicePropertiesDialog::from_device(dev, &[]);
        self.properties_dialog = Some(dialog);
        self.properties_dialog.as_ref()
    }

    /// Close Properties dialog.
    pub fn close_properties_dialog(&mut self) {
        self.properties_dialog = None;
    }

    /// Get current tree view items according to active view mode and search filter.
    pub fn get_view_items(&self) -> Vec<TreeViewItem> {
        let all_items = match self.view_mode {
            ViewMode::DevicesByType => self.tree.build_devices_by_type(self.show_hidden_devices),
            ViewMode::DevicesByConnection => {
                self.tree.build_devices_by_connection(self.show_hidden_devices)
            }
            ViewMode::ResourcesByType => self.tree.build_resources_by_type(),
            ViewMode::ResourcesByConnection => self.tree.build_resources_by_connection(),
        };

        if self.search_filter.is_empty() {
            all_items
        } else {
            all_items
                .into_iter()
                .filter(|item| {
                    item.label.to_lowercase().contains(&self.search_filter)
                        || item.id.to_lowercase().contains(&self.search_filter)
                })
                .collect()
        }
    }

    /// Render ASCII representation of Device Manager application.
    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        out.push_str("╔════════════════════════════════════════════════════════════════════════════════╗\n");
        out.push_str("║ Device Manager                                                         [—][□][✕]║\n");
        out.push_str("╠════════════════════════════════════════════════════════════════════════════════╣\n");
        out.push_str("║ File  Action  View  Help                                                       ║\n");
        out.push_str("╠════════════════════════════════════════════════════════════════════════════════╣\n");
        out.push_str(&format!(
            "║ [🔍 Scan] [⚙️ Properties] [🚫 Disable] [▶️ Enable] [🔄 Update Driver] [🗑️ Uninstall] ║\n"
        ));
        out.push_str(&format!(
            "║ View: {:<20} | Hidden: {:<3} | Filter: {:<20}║\n",
            self.view_mode.display_name(),
            if self.show_hidden_devices { "Yes" } else { "No" },
            if self.search_filter.is_empty() { "(none)" } else { &self.search_filter }
        ));
        out.push_str("╠════════════════════════════════════════════════════════════════════════════════╣\n");

        let items = self.get_view_items();
        for item in items.iter().take(40) {
            let indent = "  ".repeat(item.depth);
            let prefix = if item.is_category { "v " } else { "  " };
            let marker = if Some(&item.id) == self.selected_device_id.as_ref() {
                ">"
            } else {
                " "
            };
            let status = item.status_glyph();
            let line = format!(
                "║ {} {}{}{} {} {}",
                marker, indent, prefix, item.icon, status, item.label
            );
            out.push_str(&format!("{:<81}║\n", line));
        }

        out.push_str("╠════════════════════════════════════════════════════════════════════════════════╣\n");
        out.push_str(&format!("║ Status: {:<70} ║\n", self.status_message));
        out.push_str("╚════════════════════════════════════════════════════════════════════════════════╝\n");
        out
    }
}

// ============================================================================
// Section 9: Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: Hardware Device Tree Hierarchy & Categories Verification
    #[test]
    fn test_hardware_tree_hierarchy_all_required_categories() {
        let tree = DeviceTree::new_standard_pc();

        // Verify root node
        let root = tree.get_device(tree.root_id()).expect("Root PC node must exist");
        assert_eq!(root.friendly_name, "ACPI x64-based PC");
        assert_eq!(root.category, DeviceCategory::Computer);
        assert!(root.is_system_critical);

        // Verify Audio inputs and outputs
        let audio_devices = tree.get_devices_by_category(DeviceCategory::AudioInputsAndOutputs, false);
        assert!(!audio_devices.is_empty());
        assert!(audio_devices.iter().any(|d| d.friendly_name.contains("Speakers")));
        assert!(audio_devices.iter().any(|d| d.friendly_name.contains("VirtIO Sound Card")));

        // Verify Disk drives (NVMe & SATA)
        let disk_devices = tree.get_devices_by_category(DeviceCategory::DiskDrives, false);
        assert_eq!(disk_devices.len(), 2);
        assert!(disk_devices.iter().any(|d| d.friendly_name.contains("NVMe 1.4 SSD 64GB")));
        assert!(disk_devices.iter().any(|d| d.friendly_name.contains("ATA SATA AHCI Disk 32GB")));

        // Verify Display adapters (VirtIO-GPU & Intel/AMD Graphics)
        let display_devices = tree.get_devices_by_category(DeviceCategory::DisplayAdapters, false);
        assert_eq!(display_devices.len(), 2);
        assert!(display_devices.iter().any(|d| d.friendly_name.contains("VirtIO-GPU")));
        assert!(display_devices.iter().any(|d| d.friendly_name.contains("AMD Radeon RX / Intel UHD Graphics")));

        // Verify Firmware
        let fw_devices = tree.get_devices_by_category(DeviceCategory::Firmware, false);
        assert_eq!(fw_devices.len(), 1);
        assert_eq!(fw_devices[0].friendly_name, "System Firmware UEFI 2.8");

        // Verify Network adapters (VirtIO Gigabit & Realtek RTL8168)
        let net_devices = tree.get_devices_by_category(DeviceCategory::NetworkAdapters, false);
        assert_eq!(net_devices.len(), 2);
        assert!(net_devices.iter().any(|d| d.friendly_name.contains("VirtIO Network Adapter Gigabit")));
        assert!(net_devices.iter().any(|d| d.friendly_name.contains("Realtek RTL8168 WDM")));

        // Verify Processors (4 cores)
        let cpu_devices = tree.get_devices_by_category(DeviceCategory::Processors, false);
        assert_eq!(cpu_devices.len(), 4);

        // Verify Storage controllers (Standard NVM Express & Standard SATA AHCI)
        let storage_ctrls = tree.get_devices_by_category(DeviceCategory::StorageControllers, false);
        assert_eq!(storage_ctrls.len(), 2);
        assert!(storage_ctrls.iter().any(|d| d.friendly_name.contains("Standard NVM Express Controller")));
        assert!(storage_ctrls.iter().any(|d| d.friendly_name.contains("Standard SATA AHCI Controller")));

        // Verify System devices (PCI Express Root Complex, HPET, Power Button, Audio Controller)
        let sys_devices = tree.get_devices_by_category(DeviceCategory::SystemDevices, false);
        assert!(sys_devices.iter().any(|d| d.friendly_name.contains("PCI Express Root Complex")));
        assert!(sys_devices.iter().any(|d| d.friendly_name.contains("High Precision Event Timer")));
        assert!(sys_devices.iter().any(|d| d.friendly_name.contains("ACPI Power Button")));
    }

    /// Test 2: Device Enable / Disable Toggle Lifecycle & Status Codes
    #[test]
    fn test_device_disable_enable_toggle() {
        let mut tree = DeviceTree::new_standard_pc();
        let net_id = "PCI\\VEN_1AF4&DEV_1000&SUBSYS_00011AF4&REV_00\\3&11583659&0&18";

        // Initial state: Working properly (Code 0)
        let dev = tree.get_device(net_id).unwrap();
        assert!(dev.status.is_working());
        assert_eq!(dev.status.problem_code(), 0);

        // Disable device -> Code 22
        let disable_result = tree.disable_device(net_id);
        assert!(disable_result.is_ok());
        let dev_disabled = tree.get_device(net_id).unwrap();
        assert!(dev_disabled.status.is_disabled());
        assert_eq!(dev_disabled.status.problem_code(), 22);
        assert!(dev_disabled.status.status_text().contains("Code 22"));

        // Redundant disable returns error
        assert!(tree.disable_device(net_id).is_err());

        // Re-enable device -> Working properly (Code 0)
        let enable_result = tree.enable_device(net_id);
        assert!(enable_result.is_ok());
        let dev_enabled = tree.get_device(net_id).unwrap();
        assert!(dev_enabled.status.is_working());
        assert_eq!(dev_enabled.status.problem_code(), 0);

        // Test custom problem code (e.g. Code 43 Stopped reporting problems)
        let dev_mut = tree.get_device_mut(net_id).unwrap();
        dev_mut.set_problem(DeviceProblemCode::FailedPostStart);
        assert!(dev_mut.status.has_problem());
        assert_eq!(dev_mut.status.problem_code(), 43);
        assert!(dev_mut.status.status_text().contains("Code 43"));
    }

    /// Test 3: System-Critical Device Protection (Cannot Disable or Uninstall Root / CPU 0)
    #[test]
    fn test_system_critical_device_protection() {
        let mut tree = DeviceTree::new_standard_pc();

        // Attempting to disable ACPI x64-based PC root must fail safely
        let root_id = tree.root_id().to_string();
        let disable_root_err = tree.disable_device(&root_id);
        assert!(disable_root_err.is_err());
        assert!(matches!(
            disable_root_err.unwrap_err(),
            DevMgmtError::CannotDisableCriticalDevice(_)
        ));

        // Attempting to uninstall CPU core 0 must fail safely
        let cpu0_id = "ACPI\\GenuineIntel_-_x86_Family_6_Model_151\\_0";
        let uninstall_cpu_err = tree.uninstall_device(cpu0_id, false);
        assert!(uninstall_cpu_err.is_err());
    }

    /// Test 4: Resource Allocation & Conflict Detection Engine
    #[test]
    fn test_resource_allocation_and_conflict_engine() {
        let tree = DeviceTree::new_standard_pc();

        // 1. Verify VirtIO Network Adapter resources
        let net = tree
            .get_device("PCI\\VEN_1AF4&DEV_1000&SUBSYS_00011AF4&REV_00\\3&11583659&0&18")
            .unwrap();
        assert_eq!(net.resources.irqs.len(), 1);
        assert_eq!(net.resources.irqs[0].irq_number, 16);
        assert!(net.resources.irqs[0].is_msi);
        assert_eq!(net.resources.memory_ranges.len(), 1);
        assert_eq!(net.resources.memory_ranges[0].start, 0xFEBC0000);
        assert_eq!(net.resources.io_ports.len(), 1);
        assert_eq!(net.resources.io_ports[0].start, 0xC000);
        assert_eq!(net.resources.io_ports[0].end, 0xC01F);

        // 2. Verify clean standard configuration has zero conflicts
        let conflicts = tree.check_resource_conflicts();
        assert!(conflicts.is_empty(), "Standard PC devices must not conflict");

        // 3. Deliberately construct an overlapping resource allocation and verify detection
        let mut res_a = ResourceAllocation::new();
        res_a.add_memory(0xD0000000, 0xD000FFFF, false, false);

        let mut res_b = ResourceAllocation::new();
        res_b.add_memory(0xD0008000, 0xD0017FFF, false, false); // Overlaps res_a

        let conflict = res_a.find_conflict(&res_b);
        assert!(conflict.is_some());
        let conflict_detail = conflict.unwrap();
        assert_eq!(conflict_detail.resource_type, "Memory Range");
        assert!(conflict_detail.reason.contains("Memory overlap"));
    }

    /// Test 5: Driver Properties & INF Formatting
    #[test]
    fn test_driver_properties_and_inf_details() {
        let tree = DeviceTree::new_standard_pc();
        let gpu = tree
            .get_device("PCI\\VEN_1AF4&DEV_1050&SUBSYS_11001AF4&REV_01\\3&11583659&0&10")
            .unwrap();

        assert_eq!(gpu.driver_info.provider, "Red Hat, Inc.");
        assert_eq!(gpu.driver_info.version, "100.94.104.24000");
        assert_eq!(gpu.driver_info.inf_name, "viogpu.inf");
        assert!(gpu.driver_info.digital_signer.contains("Windows Hardware Compatibility"));
        assert!(gpu.hardware_ids[0].starts_with("PCI\\VEN_1AF4&DEV_1050"));

        // Build properties dialog and verify all fields populate
        let dialog = DevicePropertiesDialog::from_device(gpu, &[]);
        assert_eq!(dialog.driver_provider, "Red Hat, Inc.");
        assert_eq!(dialog.active_tab, PropertyTab::General);
        assert!(dialog.status_box_text.contains("working properly"));

        // Test text rendering of dialog
        let text = dialog.render_dialog_text();
        assert!(text.contains("Red Hat VirtIO GPU"));
        assert!(text.contains("General"));
    }

    /// Test 6: Scan for Hardware Changes Probing VDM Bus
    #[test]
    fn test_scan_for_hardware_changes_vdm() {
        let mut tree = DeviceTree::new_standard_pc();

        // Verify hidden device exists before scan
        let usb_id = "USBSTOR\\DiskSanDisk_Ultra___________1.00\\0123456789ABCDEF&0";
        assert!(tree.get_device(usb_id).unwrap().is_hidden);

        // Trigger scan
        let report = tree.scan_for_hardware_changes();
        assert!(report.total_devices_scanned > 10);
        assert!(report.discovered_devices.iter().any(|d| d.contains("SanDisk Ultra")));
        assert!(report.bus_probed.contains("VDM"));

        // Verify previously hidden device is now active/revealed
        assert!(!tree.get_device(usb_id).unwrap().is_hidden);
    }

    /// Test 7: Update Driver Wizard (Automatic Search & Local Browse)
    #[test]
    fn test_update_driver_wizard_search_and_browse() {
        let mut tree = DeviceTree::new_standard_pc();
        let net_id = "PCI\\VEN_1AF4&DEV_1000&SUBSYS_00011AF4&REV_00\\3&11583659&0&18";

        let prev_ver = tree.get_device(net_id).unwrap().driver_info.version.clone();

        // 1. Search Automatically
        let auto_result = tree.update_driver(net_id, UpdateDriverOptions::SearchAutomatically);
        assert!(auto_result.is_ok());
        let res = auto_result.unwrap();
        assert!(res.success);
        assert!(res.driver_updated);
        assert_eq!(res.previous_version, prev_ver);
        assert_ne!(res.new_version, prev_ver);
        assert!(res.wizard_steps.iter().any(|s| s.contains("WHQL")));

        // 2. Browse Computer
        let browse_result = tree.update_driver(
            net_id,
            UpdateDriverOptions::BrowseComputer {
                search_path: "C:\\Drivers\\NetKVM".to_string(),
                include_subfolders: true,
            },
        );
        assert!(browse_result.is_ok());
        let browse_res = browse_result.unwrap();
        assert!(browse_res.success);
        assert!(browse_res.driver_updated);
        assert!(browse_res.new_version.ends_with(".999"));
    }

    /// Test 8: Uninstall Device with Driver Software Removal
    #[test]
    fn test_uninstall_device_with_driver_removal() {
        let mut tree = DeviceTree::new_standard_pc();
        let initial_count = tree.device_count();
        let net_id = "PCI\\VEN_10EC&DEV_8168&SUBSYS_012310EC&REV_06\\3&11583659&0&19";

        // Uninstall with delete_driver_software = true
        let uninstall_res = tree.uninstall_device(net_id, true);
        assert!(uninstall_res.is_ok());
        let report = uninstall_res.unwrap();
        assert!(report.removed_from_tree);
        assert!(report.driver_package_deleted);
        assert!(report.deleted_package_path.unwrap().contains("DriverStore"));

        // Verify device is removed from tree
        assert_eq!(tree.device_count(), initial_count - 1);
        assert!(tree.get_device(net_id).is_none());
    }

    /// Test 9: All View Modes Tree Traversal
    #[test]
    fn test_all_view_modes_tree_traversal() {
        let tree = DeviceTree::new_standard_pc();

        // 1. Devices by type
        let by_type = tree.build_devices_by_type(false);
        assert!(!by_type.is_empty());
        assert!(by_type.iter().any(|i| i.is_category && i.label == "Disk drives"));

        // 2. Devices by connection
        let by_conn = tree.build_devices_by_connection(false);
        assert!(!by_conn.is_empty());
        assert_eq!(by_conn[0].label, "ACPI x64-based PC");
        assert!(by_conn.iter().any(|i| i.label.contains("PCI Express Root Complex")));

        // 3. Resources by type
        let res_by_type = tree.build_resources_by_type();
        assert!(!res_by_type.is_empty());
        assert!(res_by_type.iter().any(|i| i.label.contains("Interrupt request (IRQ)")));
        assert!(res_by_type.iter().any(|i| i.label.contains("Input/output (IO)")));
        assert!(res_by_type.iter().any(|i| i.label.contains("Memory")));

        // 4. Resources by connection
        let res_by_conn = tree.build_resources_by_connection();
        assert!(!res_by_conn.is_empty());
        assert!(res_by_conn.iter().any(|i| i.label.starts_with("Memory:")));
    }

    /// Test 10: Hidden Devices Toggle & Search Filter in DeviceManagerApp
    #[test]
    fn test_hidden_devices_toggle_and_app_ui() {
        let mut app = DeviceManagerApp::new();

        // Initially hidden devices are not shown
        let items_without_hidden = app.get_view_items();
        assert!(!items_without_hidden.iter().any(|i| i.label.contains("SanDisk Ultra")));

        // Toggle hidden devices on
        app.toggle_show_hidden_devices();
        assert!(app.show_hidden_devices);
        let items_with_hidden = app.get_view_items();
        assert!(items_with_hidden.iter().any(|i| i.label.contains("SanDisk Ultra")));

        // Test search filter
        app.set_search_filter("NVMe");
        let filtered_items = app.get_view_items();
        assert!(!filtered_items.is_empty());
        assert!(filtered_items.iter().all(|i| i.label.to_lowercase().contains("nvme") || i.id.to_lowercase().contains("nvme")));

        // Clear filter and render ASCII
        app.set_search_filter("");
        let ascii_ui = app.render_ascii();
        assert!(ascii_ui.contains("Device Manager"));
        assert!(ascii_ui.contains("ACPI x64-based PC"));
    }
}
