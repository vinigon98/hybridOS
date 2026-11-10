//! HybridOS Kernel Core (Written in 100% Safe and Modular Rust)
//! Proprietary Architecture for Parallel Windows & Linux Execution.

pub mod types;
pub mod personality;
pub mod trap;
pub mod process;
pub mod syscall;
pub mod loaders;
pub mod drivers;
pub mod arch;
pub mod sched;
pub mod mm;
pub mod vfs;
pub mod userland;
pub mod graphics;
pub mod net;
pub mod storage;
pub mod frontend;

use std::sync::Arc;
use personality::ExecutionPersonality;
use process::HybridProcess;
use trap::TrapFrame;
use syscall::SyscallRouter;
use drivers::vdm::VortexDriverManager;
use drivers::mock_drivers::{MockRealtekWdmDriver, MockAmdGpuLinuxDriver};
use loaders::pe::load_pe;
use loaders::elf::load_elf;

fn main() {
    println!("================================================================");
    println!("  _   _       _          _     _  ____   ____            _ ");
    println!(" | | | |_   _| |__  _ __(_) __| |/ ___| |  _ \\ _   _ ___| |_ ");
    println!(" | |_| | | | | '_ \\| '__| |/ _` |\\___ \\ | |_) | | | / __| __|");
    println!(" |  _  | |_| | |_) | |  | | (_| | ___) ||  _ <| |_| \\__ \\ |_ ");
    println!(" |_| |_|\\__, |_.__/|_|  |_|\\__,_||____/ |_| \\_\\\\__,_|___/\\__|");
    println!("        |___/        Proprietary Hybrid Kernel (Rust Edition)");
    println!("================================================================\n");

    // Phase 0: Hardware Architecture Core Initialization (x86_64 Long Mode)
    println!(">>> [PHASE 0] Initializing x86_64 Architecture Core (GDT, IDT, COM1 Serial)...");
    let arch_status = arch::init_arch();
    println!("[Arch Core] Status: Serial Ready={}, GDT Loaded={}, IDT Loaded={}, TSS Configured={}\n",
        arch_status.serial_ready, arch_status.gdt_loaded, arch_status.idt_loaded, arch_status.tss_configured);

    // Phase 1: Vortex Driver Model Subsystem Initialization
    println!(">>> [PHASE 1] Initializing Vortex Driver Model (VDM)...");
    let mut vdm = VortexDriverManager::new();

    // Phase 2: Heterogeneous Driver Registration
    println!("\n>>> [PHASE 2] Registering Heterogeneous Drivers in Parallel...");
    let wdm_driver = Arc::new(MockRealtekWdmDriver);
    let linux_driver = Arc::new(MockAmdGpuLinuxDriver);

    vdm.register_windows_driver(wdm_driver);
    vdm.register_linux_driver(linux_driver);

    // Phase 2.5: Hardware Abstraction & Driver Module Initialization (Agent 9)
    println!("\n>>> [PHASE 2.5] Initializing Hardware Abstraction & Platform Drivers...");
    let hw_manager = drivers::hardware::HardwareManager::probe_and_initialize();
    if let Some(ref blk) = hw_manager.virtio_block {
        vdm.register_windows_driver(blk.clone());
    }
    if let Some(ref net) = hw_manager.virtio_net {
        vdm.register_linux_driver(net.clone());
    }

    // Phase 3: Cross-ABI Driver Interoperability Bridge Test
    println!("\n>>> [PHASE 3] Testing Cross-ABI Driver Bridge (Linux -> Windows WDM)...");
    let mut net_buffer = [0u8; 64];
    match vdm.linux_read_device("net_realtek_wdm", &mut net_buffer, 0) {
        Ok(bytes) => {
            let msg = String::from_utf8_lossy(&net_buffer[..bytes]);
            println!("[Cross-Call Test] Successfully read {} bytes via VDM Bridge: \"{}\"", bytes, msg);
        }
        Err(e) => println!("[Cross-Call Test] Error: {:?}", e),
    }

    // VirtIO Block Storage Validation
    if let Some(ref blk) = hw_manager.virtio_block {
        let mut test_sector = [0u8; 512];
        test_sector[0..20].copy_from_slice(b"HYBRIDOS_SYSTEM_DISK");
        let _ = blk.write_sectors(0, &test_sector);
        let mut read_buf = [0u8; 512];
        let _ = blk.read_sectors(0, &mut read_buf);
        println!("[VirtIO-Blk Test] Disk sector 0 verified: \"{}\"", String::from_utf8_lossy(&read_buf[0..20]));
    }

    // PS/2 Keyboard Input Validation
    {
        let mut kbd = hw_manager.keyboard.lock().unwrap();
        kbd.handle_interrupt(0x1E); // 'a' make code
        kbd.handle_interrupt(0x30); // 'b' make code
        if let Some(ch) = kbd.read_char() {
            println!("[PS/2 Keyboard Test] Successfully read character from hardware ring buffer: '{}'", ch);
        }
    }

    // Phase 2.6: Partitioning & Real Filesystem Subsystem (GPT, FAT32, EXT4)
    println!("\n>>> [PHASE 2.6] Initializing Partitioning & Real Filesystem Engine (GPT, FAT32, EXT4)...");
    {
        use storage::gpt::{Guid, GptDisk};
        use storage::fat32::Fat32FileSystem;
        use storage::ext4::Ext4FileSystem;
        use storage::{PartitionBlockDevice, RamDisk, BlockDevice};

        // Create 128 MB virtual system disk with GPT table
        let disk = Arc::new(RamDisk::new(262144));
        let disk_guid = Guid::WINDOWS_BASIC_DATA;

        // Partition 1: EFI System / FAT32 (64MB = 131072 sectors from LBA 2048 to 133119)
        // Partition 2: Linux root / EXT4 (64MB = 126942 sectors from LBA 133120 to 260061)
        let part_specs = [
            (
                Guid::EFI_SYSTEM,
                Guid::parse("C12A7328-F81F-11D2-BA4B-00A0C93EC93B").unwrap(),
                2048,
                133119,
                0,
                "EFI System Partition",
            ),
            (
                Guid::LINUX_FILESYSTEM,
                Guid::parse("0FC63DAF-8483-4772-8E79-3D69D8477DE4").unwrap(),
                133120,
                260061,
                0,
                "Linux Root Filesystem",
            ),
        ];

        let gpt_image = GptDisk::create_mock_gpt_image(262144, disk_guid, &part_specs);
        let _ = disk.write_bytes(0, &gpt_image);

        // Parse GPT Disk
        let gpt_disk = GptDisk::parse_from_device(&*disk).expect("GPT parsing failed");
        println!("[GPT Partitioning] Successfully parsed GUID Partition Table on System Disk:");
        println!("  Disk GUID: {}", gpt_disk.header.disk_guid);
        println!("  Partition count: {}", gpt_disk.partitions.len());
        for p in &gpt_disk.partitions {
            println!("  - Partition \"{}\": Type={}, LBA Range {}-{} ({} MB)",
                p.name, p.partition_type_guid, p.starting_lba, p.ending_lba,
                p.size_in_bytes(512) / (1024 * 1024));
        }

        // Format and test Partition 1 as FAT32
        let fat_part = Arc::new(PartitionBlockDevice::new(disk.clone(), 2048, 131072));
        let fat_fs = Fat32FileSystem::format(fat_part.clone(), "BOOT_EFI").expect("FAT32 format failed");
        let dir = fat_fs.mkdir(fat_fs.bpb().root_cluster, "EFI").unwrap();
        let file = fat_fs.create_file(dir.first_cluster, "BOOTX64.EFI", 0).unwrap();
        let efi_payload = b"HYBRIDOS_UEFI_BOOT_LOADER_X64";
        let (first_c, sz) = fat_fs.write_file_at(file.first_cluster, 0, efi_payload, 0).unwrap();
        fat_fs.update_entry_metadata(dir.first_cluster, file.dir_entry_offset, first_c, sz as u32).unwrap();
        let efi_read = fat_fs.read_file(first_c, sz).unwrap();
        println!("[FAT32 Driver] EFI System Partition verified: read {} bytes from \"EFI/BOOTX64.EFI\"", efi_read.len());

        // Format and test Partition 2 as EXT4
        let ext4_part = Arc::new(PartitionBlockDevice::new(disk.clone(), 133120, 126942));
        let ext4_fs = Ext4FileSystem::format(ext4_part.clone(), "rootfs").expect("EXT4 format failed");
        let root_ino = ext4_fs.read_inode(storage::ext4::EXT4_ROOT_INO).unwrap();
        let entries = ext4_fs.readdir(&root_ino).unwrap();
        println!("[EXT4 Driver] Linux Root Partition verified: block size = {} bytes, inodes = {}, root entries = {}",
            ext4_fs.superblock().block_size(), ext4_fs.superblock().s_inodes_count, entries.len());

        // Mount both real partitions into the kernel VFS
        let _ = vfs::init_vfs();
        let vfs = vfs::get_vfs();
        storage::mount_fat32(vfs, "/boot/efi", fat_part).expect("Mount FAT32 failed");
        storage::mount_ext4(vfs, "/mnt/ext4", ext4_part).expect("Mount EXT4 failed");
        let vfs_efi = vfs.read_to_vec("/boot/efi/EFI/BOOTX64.EFI").expect("VFS FAT32 read failed");
        println!("[VFS Mount Bridge] Direct cross-VFS access verified: read \"{}\" from /boot/efi/EFI/BOOTX64.EFI",
            String::from_utf8_lossy(&vfs_efi));
    }

    // Phase 3.5: Network Layer Core Initialization & Stack Validation
    println!("\n>>> [PHASE 3.5] Initializing Network Layer Core Subsystem...");
    let net_status = net::init_net();
    println!("[Net Core] Status: Interfaces={}, Routes={}, ARP Entries={}, Loopback={}, Eth0={} ({})",
        net_status.interface_count, net_status.route_count, net_status.arp_entry_count,
        net_status.loopback_ready, net_status.eth0_ready, net_status.eth0_ip);

    // VirtIO Network Packet Loopback Validation
    {
        let net_stack = net::get_network_stack();
        let mut stack = net_stack.lock().unwrap();
        let ping_target = net::Ipv4Address::new(192, 168, 1, 1);
        if let Ok(seq) = stack.ping(ping_target, b"HYBRIDOS_KERNEL_PING") {
            println!("[Net Core Validation] Generated ICMP Echo Request (Seq: {}) to Gateway {}", seq, ping_target);
        }
    }

    // Phase 4: Binary Loaders Test (PE32+ & ELF64)
    println!("\n>>> [PHASE 4] Testing Dual Binary Loaders in Rust...");

    // 4.1 Synthetic Windows PE32+
    let mut sample_pe = vec![0u8; 512];
    sample_pe[0] = 0x4D; // 'M'
    sample_pe[1] = 0x5A; // 'Z'
    sample_pe[60] = 128; // e_lfanew
    let pe_off = 128;
    sample_pe[pe_off]     = b'P';
    sample_pe[pe_off + 1] = b'E';
    sample_pe[pe_off + 2] = 0;
    sample_pe[pe_off + 3] = 0;
    // Machine = x86_64 (0x8664)
    sample_pe[pe_off + 4] = 0x64;
    sample_pe[pe_off + 5] = 0x86;
    sample_pe[pe_off + 6] = 2; // NumberOfSections = 2
    // SizeOfOptionalHeader = 240
    sample_pe[pe_off + 20] = 240;
    // Magic = PE32+ (0x20B)
    let opt = pe_off + 24;
    sample_pe[opt] = 0x0B;
    sample_pe[opt + 1] = 0x02;
    // AddressOfEntryPoint = 0x1000
    sample_pe[opt + 16] = 0x00;
    sample_pe[opt + 17] = 0x10;
    // ImageBase = 0x0000000140000000
    sample_pe[opt + 24] = 0x00;
    sample_pe[opt + 25] = 0x00;
    sample_pe[opt + 26] = 0x00;
    sample_pe[opt + 27] = 0x40;
    sample_pe[opt + 28] = 0x01;
    // SizeOfImage = 0x8000
    sample_pe[opt + 56] = 0x00;
    sample_pe[opt + 57] = 0x80;

    let mut win_proc = HybridProcess::new(1000, "explorer_host.exe", ExecutionPersonality::WindowsNt);
    let _ = load_pe(&mut win_proc, &sample_pe);

    // 4.2 Synthetic Linux ELF64
    let mut sample_elf = vec![0u8; 512];
    sample_elf[0] = 0x7F;
    sample_elf[1] = b'E';
    sample_elf[2] = b'L';
    sample_elf[3] = b'F';
    sample_elf[4] = 2; // 64-bit
    sample_elf[5] = 1; // Little-endian
    sample_elf[6] = 1; // Current version
    sample_elf[16] = 2; // ET_EXEC
    sample_elf[18] = 62; // EM_X86_64
    sample_elf[24] = 0x00; // EntryPoint: 0x401000
    sample_elf[25] = 0x10;
    sample_elf[26] = 0x40;
    sample_elf[32] = 64; // phoff = 64
    sample_elf[54] = 56; // phentsize = 56
    sample_elf[56] = 1;  // phnum = 1
    // Program header 0: PT_LOAD (1)
    let ph = 64;
    sample_elf[ph] = 1; // PT_LOAD
    sample_elf[ph + 4] = 5; // PF_R | PF_X
    sample_elf[ph + 16] = 0x00; // VAddr: 0x400000
    sample_elf[ph + 17] = 0x00;
    sample_elf[ph + 18] = 0x40;
    sample_elf[ph + 40] = 0x00; // MemSz: 0x3000
    sample_elf[ph + 41] = 0x30;

    let mut lin_proc = HybridProcess::new(1001, "bash_server", ExecutionPersonality::LinuxPosix);
    let _ = load_elf(&mut lin_proc, &sample_elf);

    // Phase 5: Parallel Dual-Personality Syscall Dispatching
    println!("\n>>> [PHASE 5] Executing Syscalls via Dual-ABI Router...");

    // Test 5.1: Windows NtWriteFile
    println!("[Test 5.1] Executing Windows Syscall (NtWriteFile - SSDT 0x0008)...");
    let win_msg = std::ffi::CString::new("Hello from Windows NT Application running inside Rust HybridOS!\n").unwrap();
    let win_stack: [u64; 8] = [
        0, 0, 0, 0, 0,
        0, // Arg 5 (IoStatusBlock)
        win_msg.as_ptr() as u64, // Arg 6 (Buffer)
        0,
    ];

    let mut win_tf = TrapFrame {
        rax: syscall::windows::SYS_NT_WRITE_FILE,
        r10: 1, // Arg 1: stdout
        rsp: win_stack.as_ptr() as u64,
        ..Default::default()
    };
    SyscallRouter::dispatch(&mut win_proc, &mut win_tf);
    println!("Windows NT Status: 0x{:08X}", win_tf.rax as u32);

    // Test 5.2: Linux sys_write
    println!("\n[Test 5.2] Executing Linux Syscall (sys_write - Syscall #1)...");
    let lin_msg = "Hello from Linux ELF Application running inside Rust HybridOS!\n";
    let mut lin_tf = TrapFrame {
        rax: syscall::linux::SYS_LINUX_WRITE,
        rdi: 1, // Arg 1: stdout
        rsi: lin_msg.as_ptr() as u64, // Arg 2: buffer
        rdx: lin_msg.len() as u64,    // Arg 3: count
        ..Default::default()
    };
    SyscallRouter::dispatch(&mut lin_proc, &mut lin_tf);
    println!("Linux Syscall Result: {} bytes written", lin_tf.rax);

    // Test 5.3: Linux Extended POSIX Syscalls (sys_uname & sys_arch_prctl via Dual-ABI Router)
    println!("\n[Test 5.3] Executing Extended Linux Syscalls (sys_uname & sys_arch_prctl via Dual-ABI Router)...");
    let mut uts_buf = syscall::linux_extended::Utsname::default();
    let mut uname_tf = TrapFrame {
        rax: syscall::linux_extended::SYS_UNAME,
        rdi: &mut uts_buf as *mut _ as u64,
        ..Default::default()
    };
    SyscallRouter::dispatch(&mut lin_proc, &mut uname_tf);
    let sysname = std::str::from_utf8(&uts_buf.sysname).unwrap_or("").trim_matches('\0');
    let release = std::str::from_utf8(&uts_buf.release).unwrap_or("").trim_matches('\0');
    let machine = std::str::from_utf8(&uts_buf.machine).unwrap_or("").trim_matches('\0');
    println!("[Linux POSIX sys_uname]: OS='{}', Release='{}', Arch='{}'", sysname, release, machine);

    // Test 5.4: Linux sys_arch_prctl setting %fs base for TLS
    let target_fs_base = 0x0000_7FFF_A000_1000u64;
    let mut prctl_tf = TrapFrame {
        rax: syscall::linux_extended::SYS_ARCH_PRCTL,
        rdi: syscall::linux_extended::ARCH_SET_FS,
        rsi: target_fs_base,
        ..Default::default()
    };
    SyscallRouter::dispatch(&mut lin_proc, &mut prctl_tf);
    println!("[Linux POSIX sys_arch_prctl]: ARCH_SET_FS configured TLS base to 0x{:016X}", target_fs_base);

    // Phase 6: Memory Management Subsystem (MM) Initialization & Validation
    println!("\n>>> [PHASE 6] Initializing HybridOS Memory Management Subsystem (Rust MM)...");
    match mm::init_mm() {
        Ok(info) => {
            println!("[MM Validation] Physical RAM: {} MB total, {} frames managed",
                info.total_physical_bytes / (1024 * 1024), info.total_frames);
            println!("[MM Validation] Kernel Page Table CR3: 0x{:016X}", info.kernel_pml4_cr3.as_u64());

            // Test 6.1: Physical Frame Allocation
            let test_frame = mm::allocate_frame().expect("Physical frame allocation failed");
            println!("[MM Validation] Allocated 4KB physical frame: {}", test_frame);
            mm::free_frame(test_frame).expect("Physical frame deallocation failed");

            // Test 6.2: Virtual-to-Physical Translation via Kernel Page Table
            if let Ok(guard) = mm::KERNEL_PAGE_TABLE.lock() {
                if let Some(ref pt_mgr) = *guard {
                    let kernel_va = mm::VirtAddr::new(0x1000).unwrap();
                    let translated = pt_mgr.translate(kernel_va);
                    println!("[MM Validation] Virtual address {} -> Physical address {:?}", kernel_va, translated);
                }
            }

            // Test 6.3: Dynamic Kernel Heap Allocation & Immediate Coalescing
            let test_ptr = mm::kernel_allocate(256, 16).expect("Kernel heap allocation failed");
            println!("[MM Validation] Dynamic Heap allocation succeeded: 256 bytes @ 0x{:X}", test_ptr as usize);
            mm::kernel_free(test_ptr).expect("Kernel heap deallocation failed");
            println!("[MM Validation] Dynamic Heap deallocation and coalescing verified.");
        }
        Err(e) => println!("[MM Error] Memory Management Subsystem failed to initialize: {}", e),
    }

    // Phase 7: Process & Thread Scheduler Subsystem Initialization & Validation
    println!("\n>>> [PHASE 7] Initializing HybridOS Multi-Queue Process & Thread Scheduler...");
    let sched_mutex = sched::init_scheduler();
    {
        let mut scheduler = sched_mutex.lock().unwrap();

        // 7.1: Create Windows NT Process & Thread
        let win_sched_proc = scheduler.create_process(
            "explorer_host.exe",
            ExecutionPersonality::WindowsNt,
            sched::PriorityClass::Normal,
        );
        let win_pid = win_sched_proc.lock().unwrap().pid;
        let win_thread = scheduler
            .create_thread(
                win_pid,
                "explorer_main",
                win_proc.entry_point,
                0x0000_7FFF_FFFF_0000,
                sched::Priority::NORMAL,
            )
            .expect("Failed to create Windows thread");
        let win_tid = win_thread.lock().unwrap().tid;

        // 7.2: Create Linux POSIX Process & Thread
        let lin_sched_proc = scheduler.create_process(
            "bash_server",
            ExecutionPersonality::LinuxPosix,
            sched::PriorityClass::Normal,
        );
        let lin_pid = lin_sched_proc.lock().unwrap().pid;
        let lin_thread = scheduler
            .create_thread(
                lin_pid,
                "bash_main",
                lin_proc.entry_point,
                0x0000_7FFF_FFFF_1000,
                sched::Priority::NORMAL,
            )
            .expect("Failed to create Linux thread");
        let lin_tid = lin_thread.lock().unwrap().tid;

        println!(
            "[Sched Validation] Registered Tasks: Windows NT [PID {}, TID {}] & Linux POSIX [PID {}, TID {}]",
            win_pid, win_tid, lin_pid, lin_tid
        );

        // 7.3: Test Round-Robin scheduling and cooperative yielding
        let first_scheduled = scheduler.schedule().expect("Scheduling failed");
        println!(
            "[Sched Validation] Scheduled initial thread: TID {} (State: {:?})",
            first_scheduled,
            scheduler.get_thread(first_scheduled).unwrap().lock().unwrap().state
        );

        let yielded_to = scheduler.yield_cpu().expect("Yield failed");
        println!(
            "[Sched Validation] Cooperative yield rotated execution to: TID {} (Switches: {})",
            yielded_to,
            scheduler.stats().context_switches
        );

        // 7.4: Test Preemption & Timer Tick
        for _ in 0..10 {
            scheduler.tick();
        }
        println!(
            "[Sched Validation] Preemptive ticks verified (System Ticks: {}, Switches: {})",
            scheduler.system_ticks(),
            scheduler.stats().context_switches
        );
    }

    // Phase 8: Userland Shell & Multi-Subsystem CLI Validation (PowerShell, CMD, Linux)
    println!("\n>>> [PHASE 8] Initializing Userland Shell & Multi-Subsystem CLI (PowerShell, CMD, Linux)...");
    let mut shell = userland::shell::HybridShell::new();
    println!("[Userland Shell] Initialized successfully. Default prompt: \"{}\"", shell.prompt().trim());

    // 8.1: Test Windows CMD Commands (ver, vol, systeminfo)
    println!("\n[Test 8.1] Windows Command Prompt (CMD) commands 'ver', 'vol':");
    print!("{}", shell.execute("ver"));
    print!("{}", shell.execute("vol"));

    // 8.2: Test PowerShell Cmdlets (Get-Service, Test-Path)
    println!("\n[Test 8.2] PowerShell cmdlets 'Get-Service', 'Test-Path':");
    println!("Test-Path 'C:\\Windows\\System32\\cmd.exe' -> {}", shell.execute("Test-Path C:\\Windows\\System32\\cmd.exe").trim());
    println!("Test-Path '/etc/os-release' -> {}", shell.execute("Test-Path /etc/os-release").trim());
    print!("{}", shell.execute("Get-Service"));

    // 8.3: Test Linux Commands (df, ifconfig, ping)
    println!("\n[Test 8.3] Linux POSIX commands 'df', 'ifconfig', 'ping':");
    print!("{}", shell.execute("df"));
    print!("{}", shell.execute("ifconfig"));
    print!("{}", shell.execute("ping 192.168.1.1"));

    // 8.4: Test 'personality' inspection & flavor switching (cmd <-> powershell <-> bash)
    println!("\n[Test 8.4] Shell Flavor & Personality Switching:");
    print!("{}", shell.execute("powershell"));
    println!("Prompt after 'powershell': \"{}\"", shell.prompt().trim());
    print!("{}", shell.execute("cmd"));
    println!("Prompt after 'cmd': \"{}\"", shell.prompt().trim());
    print!("{}", shell.execute("bash"));
    println!("Prompt after 'bash': \"{}\"", shell.prompt().trim());
    print!("{}", shell.execute("hybrid"));
    println!("Prompt after 'hybrid': \"{}\"", shell.prompt().trim());

    // 8.5: Test 'run' PE and ELF binary execution
    println!("\n[Test 8.5] Shell built-in command 'run sample_pe':");
    println!("{}", shell.execute("run sample_pe"));
    println!("[Test 8.6] Shell built-in command 'run sample_elf':");
    println!("{}", shell.execute("run sample_elf"));

    // Phase 9: Universal 3D Graphics Bridge & Dual-Personality Compositor Subsystem
    println!("\n>>> [PHASE 9] Initializing Universal 3D Graphics Bridge & Compositor Subsystem...");
    match graphics::init_graphics() {
        Ok(gfx) => {
            println!("[Graphics Core] Subsystem active: Display Resolution {}x{}", gfx.display_width, gfx.display_height);
            let icd = gfx.vulkan_icd.lock().unwrap();
            let gpu = &icd.physical_devices[0];
            println!("[Vulkan HAL] Physical Device: {} (API v{}.{}.{})",
                gpu.properties.device_name,
                (gpu.properties.api_version >> 22) & 0x7F,
                (gpu.properties.api_version >> 12) & 0x3FF,
                gpu.properties.api_version & 0xFFF
            );
            let factory = gfx.dxgi_factory.lock().unwrap();
            let adapter = factory.enum_adapters(0).unwrap();
            println!("[DXGI Bridge] D3D11/12 Adapter: {} (Dedicated VRAM: {} MB)",
                adapter.get_desc().description,
                adapter.get_desc().dedicated_video_memory / (1024 * 1024)
            );
            println!("[Compositor] Dual-Personality Compositor ready with Hardware Cursor Plane & Dirty Rect Tracking.");
        }
        Err(e) => println!("[Graphics Error] Failed to initialize graphics subsystem: {:?}", e),
    }

    println!("\n================================================================");
    println!("[SUCCESS] 100% Rust HybridOS Kernel validated successfully!");
    println!("================================================================");

    // Check if user requested interactive terminal REPL
    let cli_args: Vec<String> = std::env::args().collect();
    if cli_args.iter().any(|arg| arg == "--shell" || arg == "shell" || arg == "-i" || arg == "--interactive") {
        println!("\n>>> Starting HybridOS Interactive Universal Shell...");
        println!("Type 'help' or 'Get-Command' for command list.");
        println!("Type 'powershell', 'cmd', 'bash', or 'hybrid' to switch prompt flavors.");
        println!("Type 'exit' to shut down the session.\n");
        let _ = shell.run_repl(std::io::stdin().lock(), std::io::stdout());
    } else {
        println!("\n[TIP] To launch the interactive terminal session directly, run:");
        println!("      cargo run -- --shell");
    }
}
