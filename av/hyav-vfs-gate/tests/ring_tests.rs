use hyav_vfs_gate::gate::{
    ntstatus, section_flags, DegradationMode, DegradationState, GateConfig, GateEngine, Verdict,
};
use hyav_vfs_gate::mdl::{Mdl, MdlError};
use hyav_vfs_gate::ring::{
    CqEntry, RingError, SharedRingBuffer, SqEntry, RING_TOTAL_SIZE, SQ_CAPACITY,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[test]
fn test_ring_buffer_128kb_size_and_alignment() {
    assert_eq!(
        core::mem::size_of::<SharedRingBuffer>(),
        RING_TOTAL_SIZE,
        "SharedRingBuffer must be exactly 128 KB (131,072 bytes)"
    );
    assert_eq!(RING_TOTAL_SIZE, 128 * 1024);
    assert_eq!(core::mem::align_of::<SharedRingBuffer>(), 4096);
}

#[test]
fn test_sq_cq_basic_submit_and_consume() {
    let mut ring = SharedRingBuffer::new();

    assert_eq!(ring.sq_pending_count(), 0);
    assert_eq!(ring.cq_available_count(), 0);

    // Submit SQ entry
    let mut sqe = SqEntry::default();
    sqe.request_id = 42;
    sqe.pid = 1337;
    let seq = ring.sq_submit(&sqe).expect("SQ submit should succeed");
    assert_eq!(seq, 0);
    assert_eq!(ring.sq_pending_count(), 1);

    // Consume SQ entry
    let consumed_sq = ring.sq_consume().expect("SQ consume should return entry");
    assert_eq!(consumed_sq.request_id, 42);
    assert_eq!(consumed_sq.pid, 1337);
    assert_eq!(ring.sq_pending_count(), 0);
    assert!(ring.sq_consume().is_none());

    // Submit CQ entry
    let mut cqe = CqEntry::default();
    cqe.request_id = 42;
    cqe.status = ntstatus::STATUS_SUCCESS;
    cqe.verdict = Verdict::Allow as u8;
    ring.cq_submit(&cqe).expect("CQ submit should succeed");
    assert_eq!(ring.cq_available_count(), 1);

    // Consume CQ entry
    let consumed_cq = ring.cq_consume().expect("CQ consume should return entry");
    assert_eq!(consumed_cq.request_id, 42);
    assert_eq!(consumed_cq.status, ntstatus::STATUS_SUCCESS);
    assert_eq!(consumed_cq.verdict, Verdict::Allow as u8);
    assert_eq!(ring.cq_available_count(), 0);
    assert!(ring.cq_consume().is_none());
}

#[test]
fn test_sq_capacity_and_saturation() {
    let mut ring = SharedRingBuffer::new();

    // Fill the SQ ring to capacity
    for i in 0..SQ_CAPACITY {
        let mut sqe = SqEntry::default();
        sqe.request_id = i as u64;
        ring.sq_submit(&sqe).expect("Submission within capacity");
    }

    assert_eq!(ring.sq_pending_count(), SQ_CAPACITY);

    // Next submit must fail with QueueFull
    let mut overflow_sqe = SqEntry::default();
    overflow_sqe.request_id = 9999;
    assert_eq!(ring.sq_submit(&overflow_sqe), Err(RingError::QueueFull));

    // Consume one item, now submit should succeed
    let item = ring.sq_consume().expect("Should consume first item");
    assert_eq!(item.request_id, 0);

    assert!(ring.sq_submit(&overflow_sqe).is_ok());
}

#[test]
fn test_ring_u32_wrap_around() {
    let mut ring = SharedRingBuffer::new();

    // Artificially initialize tail and head near u32::MAX to verify wrapping
    let start_idx = u32::MAX - 5;
    ring.header.sq_tail.store(start_idx, Ordering::SeqCst);
    ring.header.sq_head.store(start_idx, Ordering::SeqCst);

    for i in 0..20 {
        let mut sqe = SqEntry::default();
        sqe.request_id = 1000 + i;
        ring.sq_submit(&sqe).expect("Wrapping submit must succeed");

        let consumed = ring.sq_consume().expect("Wrapping consume must succeed");
        assert_eq!(consumed.request_id, 1000 + i);
    }
}

#[test]
fn test_concurrent_producer_consumer_threads() {
    let mut ring = SharedRingBuffer::new();
    let ring_ptr = &mut *ring as *mut SharedRingBuffer as usize;

    let iterations = 10000;
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();

    // Producer thread
    let producer_handle = thread::spawn(move || {
        for i in 0..iterations {
            let mut sqe = SqEntry::default();
            sqe.request_id = i;
            unsafe {
                let ring = &mut *(ring_ptr as *mut SharedRingBuffer);
                while ring.sq_submit(&sqe).is_err() {
                    thread::yield_now();
                }
            }
        }
        done_clone.store(true, Ordering::Release);
    });

    // Consumer thread
    let consumer_handle = thread::spawn(move || {
        let mut consumed_count = 0;
        while consumed_count < iterations {
            unsafe {
                let ring = &mut *(ring_ptr as *mut SharedRingBuffer);
                if let Some(entry) = ring.sq_consume() {
                    assert_eq!(entry.request_id, consumed_count);
                    consumed_count += 1;
                } else {
                    thread::yield_now();
                }
            }
        }
        consumed_count
    });

    producer_handle.join().unwrap();
    let total_consumed = consumer_handle.join().unwrap();
    assert_eq!(total_consumed, iterations);
}

#[test]
fn test_mdl_zero_copy_projection() {
    let test_payload = b"HybridOS-Malware-Signature-ZeroCopy-Test-Payload-1234567890";
    let mut mdl = Mdl::from_slice(test_payload, 42).expect("MDL from slice");

    assert_eq!(mdl.byte_count(), test_payload.len() as u32);
    assert!(!mdl.is_locked());

    // Projecting before locking must fail with PagesNotLocked
    assert_eq!(mdl.project_zero_copy().err(), Some(MdlError::PagesNotLocked));

    // Lock pages
    mdl.probe_and_lock_pages(true).expect("Lock pages");
    assert!(mdl.is_locked());
    assert!(mdl.is_read_only());

    // Project zero-copy slice
    {
        let projection = mdl.project_zero_copy().expect("Zero-copy projection");
        assert_eq!(projection.as_slice(), test_payload);
        assert_eq!(projection.len(), test_payload.len());
        // Verify pointer equality: zero copy means exact same memory address!
        assert_eq!(projection.as_slice().as_ptr(), test_payload.as_ptr());

        // Attempting double projection while active must fail with AlreadyProjected
        assert_eq!(mdl.project_zero_copy().err(), Some(MdlError::AlreadyProjected));
    }

    // After projection is dropped, a new projection can be acquired
    assert!(mdl.project_zero_copy().is_ok());

    // Attempting mutable projection on read-only MDL must fail
    assert_eq!(
        mdl.project_zero_copy_mut().err(),
        Some(MdlError::WriteViolationReadOnly)
    );
}

#[test]
fn test_sec_image_returns_status_virus_infected_on_block() {
    let config = GateConfig {
        timeout_ms: 1000,
        max_consecutive_timeouts: 3,
        degradation_mode: DegradationMode::FailClosed,
    };
    let mut gate = GateEngine::new(config);

    // Worker simulation: thread answers in CQ ring with Block
    let ring_ptr = gate.ring_mut() as *mut SharedRingBuffer as usize;
    let worker_thread = thread::spawn(move || {
        let ring = unsafe { &mut *(ring_ptr as *mut SharedRingBuffer) };
        loop {
            if let Some(sqe) = ring.sq_consume() {
                // Return Block verdict
                let mut cqe = CqEntry::default();
                cqe.request_id = sqe.request_id;
                cqe.verdict = Verdict::Block as u8;
                cqe.status = ntstatus::STATUS_VIRUS_INFECTED;
                ring.cq_submit(&cqe).unwrap();
                break;
            }
            thread::yield_now();
        }
    });

    let status = gate.intercept_create_section(
        r"\Device\HarddiskVolume3\Malware\trojan.exe",
        section_flags::SEC_IMAGE,
        None,
        1001,
        2001,
    );

    worker_thread.join().unwrap();

    // Verify SPECIFIC REQUIREMENT: Returns STATUS_VIRUS_INFECTED (0xC0000906) on SEC_IMAGE!
    assert_eq!(
        status,
        ntstatus::STATUS_VIRUS_INFECTED,
        "SEC_IMAGE block MUST return STATUS_VIRUS_INFECTED (0xC0000906)"
    );
    assert_eq!(status, 0xC0000906);
    assert_eq!(gate.total_blocked(), 1);
}

#[test]
fn test_non_sec_image_returns_status_access_denied_on_block() {
    let gate = GateEngine::new(GateConfig::default());
    let status = gate.resolve_verdict_to_status(Verdict::Block, false);
    assert_eq!(status, ntstatus::STATUS_ACCESS_DENIED);
    assert_eq!(status, 0xC0000022);
}

#[test]
fn test_sec_image_returns_status_success_on_allow() {
    let gate = GateEngine::new(GateConfig::default());
    let status = gate.resolve_verdict_to_status(Verdict::Allow, true);
    assert_eq!(status, ntstatus::STATUS_SUCCESS);
    assert_eq!(status, 0x00000000);
}

#[test]
fn test_status_pending_async_workflow() {
    let mut gate = GateEngine::new(GateConfig::default());

    // Submit async
    let (req_id, status) = gate
        .submit_async(r"\SystemRoot\System32\notepad.exe", true, None, 400, 500)
        .expect("Async submit");

    // Must return STATUS_PENDING (0x00000103)
    assert_eq!(status, ntstatus::STATUS_PENDING);
    assert_eq!(status, 0x00000103);

    // Initial poll: not yet complete
    assert_eq!(gate.poll_async_completion(req_id, true), None);

    // Worker consumes and completes with Allow
    let sqe = gate.ring_mut().sq_consume().expect("Worker dequeues request");
    assert_eq!(sqe.request_id, req_id);

    let mut cqe = CqEntry::default();
    cqe.request_id = req_id;
    cqe.verdict = Verdict::Allow as u8;
    gate.ring_mut().cq_submit(&cqe).unwrap();

    // Polling now yields STATUS_SUCCESS
    let final_status = gate.poll_async_completion(req_id, true);
    assert_eq!(final_status, Some(ntstatus::STATUS_SUCCESS));
}

#[test]
fn test_degradation_timeout_fail_open_and_fail_closed() {
    // Test Fail-Closed with SEC_IMAGE -> STATUS_VIRUS_INFECTED
    let config_closed = GateConfig {
        timeout_ms: 20, // fast timeout for test
        max_consecutive_timeouts: 2,
        degradation_mode: DegradationMode::FailClosed,
    };
    let mut gate_closed = GateEngine::new(config_closed);

    // Nobody consumes from the ring, simulating dead worker
    let status_timeout = gate_closed.intercept_create_section(
        r"\Device\HarddiskVolume1\test.exe",
        section_flags::SEC_IMAGE,
        None,
        555,
        666,
    );
    assert_eq!(
        status_timeout,
        ntstatus::STATUS_VIRUS_INFECTED,
        "Fail-Closed SEC_IMAGE must return STATUS_VIRUS_INFECTED upon timeout"
    );
    assert_eq!(gate_closed.degradation_state(), DegradationState::Degraded);

    // Second timeout triggers Critical state
    gate_closed.intercept_create_section(
        r"\Device\HarddiskVolume1\test2.exe",
        section_flags::SEC_IMAGE,
        None,
        555,
        666,
    );
    assert_eq!(gate_closed.degradation_state(), DegradationState::Critical);

    // Test Fail-Open -> STATUS_SUCCESS
    let config_open = GateConfig {
        timeout_ms: 20,
        max_consecutive_timeouts: 2,
        degradation_mode: DegradationMode::FailOpen,
    };
    let mut gate_open = GateEngine::new(config_open);

    let status_open = gate_open.intercept_create_section(
        r"\Device\HarddiskVolume1\critical_service.exe",
        section_flags::SEC_IMAGE,
        None,
        777,
        888,
    );
    assert_eq!(status_open, ntstatus::STATUS_SUCCESS);
    assert_eq!(gate_open.total_degraded(), 1);
}
