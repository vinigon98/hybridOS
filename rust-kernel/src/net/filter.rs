//! HybridOS Network Filter Hook Subsystem (HyAV M6-1)
//!
//! Provides kernel-level interception for network operations:
//! - [`NetworkFilterHook`]: High-performance filter engine with O(1) rule evaluation.
//! - Interception points: `connect`, `bind`, `accept`, `sendto`.
//! - Context collection: PID, protocol, operation, local and remote IP/ports.
//! - Enforcement: Immediate block returning `ECONNREFUSED` / [`SocketError::ConnectionRefused`].

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use super::socket::{SocketAddress, SocketError, SocketProtocol};
use crate::types::PosixError;

/// Network operations intercepted by the filter hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetworkFilterOp {
    Connect,
    Bind,
    Accept,
    SendTo,
}

impl std::fmt::Display for NetworkFilterOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkFilterOp::Connect => write!(f, "CONNECT"),
            NetworkFilterOp::Bind => write!(f, "BIND"),
            NetworkFilterOp::Accept => write!(f, "ACCEPT"),
            NetworkFilterOp::SendTo => write!(f, "SENDTO"),
        }
    }
}

/// Inspection verdict produced by the filter hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterVerdict {
    /// Allow the operation to proceed.
    Allow,
    /// Block the operation and return ECONNREFUSED.
    Block,
}

/// Rich context collected at the interception hook point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFilterContext {
    /// Calling Process ID.
    pub pid: u32,
    /// Transport / network protocol.
    pub protocol: SocketProtocol,
    /// Operation being performed.
    pub operation: NetworkFilterOp,
    /// Local address (source IP/port) if known/bound.
    pub local_addr: Option<SocketAddress>,
    /// Remote address (destination IP/port) if applicable.
    pub remote_addr: Option<SocketAddress>,
}

impl NetworkFilterContext {
    /// Create a new inspection context.
    pub const fn new(
        pid: u32,
        protocol: SocketProtocol,
        operation: NetworkFilterOp,
        local_addr: Option<SocketAddress>,
        remote_addr: Option<SocketAddress>,
    ) -> Self {
        Self {
            pid,
            protocol,
            operation,
            local_addr,
            remote_addr,
        }
    }

    /// Source IPv4 address if present.
    #[inline]
    pub fn src_ip(&self) -> Option<[u8; 4]> {
        self.local_addr.map(|a| a.ip)
    }

    /// Source port if present.
    #[inline]
    pub fn src_port(&self) -> Option<u16> {
        self.local_addr.map(|a| a.port)
    }

    /// Destination IPv4 address if present.
    #[inline]
    pub fn dst_ip(&self) -> Option<[u8; 4]> {
        self.remote_addr.map(|a| a.ip)
    }

    /// Destination port if present.
    #[inline]
    pub fn dst_port(&self) -> Option<u16> {
        self.remote_addr.map(|a| a.port)
    }
}

/// Fast O(1) Network Filter Engine.
pub struct NetworkFilterHook {
    /// Master administrative toggle for filter evaluation.
    enabled: AtomicBool,
    /// Set of blocked Process IDs (O(1) hash lookup).
    blocked_pids: RwLock<HashSet<u32>>,
    /// Set of blocked ports (O(1) hash lookup).
    blocked_ports: RwLock<HashSet<u16>>,
    /// Set of blocked IPv4 addresses (O(1) hash lookup).
    blocked_ips: RwLock<HashSet<[u8; 4]>>,
    /// Set of blocked specific endpoints (IP + port) (O(1) hash lookup).
    blocked_endpoints: RwLock<HashSet<SocketAddress>>,
    /// Set of blocked (Operation, Endpoint) pairs (O(1) hash lookup).
    blocked_ops: RwLock<HashSet<(NetworkFilterOp, SocketAddress)>>,
    /// Set of blocked (PID, Operation) pairs (O(1) hash lookup).
    blocked_pid_ops: RwLock<HashSet<(u32, NetworkFilterOp)>>,
    /// Optional dynamic filter callback for advanced heuristic/policy evaluation.
    custom_hook: RwLock<Option<Arc<dyn Fn(&NetworkFilterContext) -> FilterVerdict + Send + Sync>>>,
}

impl NetworkFilterHook {
    /// Create a new empty filter hook instance.
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            blocked_pids: RwLock::new(HashSet::new()),
            blocked_ports: RwLock::new(HashSet::new()),
            blocked_ips: RwLock::new(HashSet::new()),
            blocked_endpoints: RwLock::new(HashSet::new()),
            blocked_ops: RwLock::new(HashSet::new()),
            blocked_pid_ops: RwLock::new(HashSet::new()),
            custom_hook: RwLock::new(None),
        }
    }

    /// Check if the filter hook is actively evaluating packets.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable filter enforcement.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    // ========================================================================
    // O(1) Rule Management
    // ========================================================================

    /// Block all network traffic from a given PID.
    pub fn block_pid(&self, pid: u32) {
        self.blocked_pids.write().unwrap().insert(pid);
    }

    /// Remove a PID from the blocked set.
    pub fn unblock_pid(&self, pid: u32) {
        self.blocked_pids.write().unwrap().remove(&pid);
    }

    /// Block all network traffic involving a given port (source or destination).
    pub fn block_port(&self, port: u16) {
        self.blocked_ports.write().unwrap().insert(port);
    }

    /// Remove a port from the blocked set.
    pub fn unblock_port(&self, port: u16) {
        self.blocked_ports.write().unwrap().remove(&port);
    }

    /// Block all network traffic involving a given IPv4 address (source or destination).
    pub fn block_ip(&self, ip: [u8; 4]) {
        self.blocked_ips.write().unwrap().insert(ip);
    }

    /// Remove an IPv4 address from the blocked set.
    pub fn unblock_ip(&self, ip: [u8; 4]) {
        self.blocked_ips.write().unwrap().remove(&ip);
    }

    /// Block a specific socket endpoint (IP + Port).
    pub fn block_endpoint(&self, addr: SocketAddress) {
        self.blocked_endpoints.write().unwrap().insert(addr);
    }

    /// Remove an endpoint from the blocked set.
    pub fn unblock_endpoint(&self, addr: SocketAddress) {
        self.blocked_endpoints.write().unwrap().remove(&addr);
    }

    /// Block a specific operation on a given target endpoint.
    pub fn block_operation(&self, op: NetworkFilterOp, addr: SocketAddress) {
        self.blocked_ops.write().unwrap().insert((op, addr));
    }

    /// Remove an operation-endpoint rule from the blocked set.
    pub fn unblock_operation(&self, op: NetworkFilterOp, addr: SocketAddress) {
        self.blocked_ops.write().unwrap().remove(&(op, addr));
    }

    /// Block a specific operation for a specific process ID.
    pub fn block_pid_operation(&self, pid: u32, op: NetworkFilterOp) {
        self.blocked_pid_ops.write().unwrap().insert((pid, op));
    }

    /// Remove a PID operation rule from the blocked set.
    pub fn unblock_pid_operation(&self, pid: u32, op: NetworkFilterOp) {
        self.blocked_pid_ops.write().unwrap().remove(&(pid, op));
    }

    /// Register a custom dynamic inspection hook.
    pub fn set_custom_hook<F>(&self, hook: F)
    where
        F: Fn(&NetworkFilterContext) -> FilterVerdict + Send + Sync + 'static,
    {
        *self.custom_hook.write().unwrap() = Some(Arc::new(hook));
    }

    /// Remove any registered custom inspection hook.
    pub fn clear_custom_hook(&self) {
        *self.custom_hook.write().unwrap() = None;
    }

    /// Clear all filtering rules and reset to allow-all.
    pub fn clear_all_rules(&self) {
        self.blocked_pids.write().unwrap().clear();
        self.blocked_ports.write().unwrap().clear();
        self.blocked_ips.write().unwrap().clear();
        self.blocked_endpoints.write().unwrap().clear();
        self.blocked_ops.write().unwrap().clear();
        self.blocked_pid_ops.write().unwrap().clear();
        *self.custom_hook.write().unwrap() = None;
    }

    // ========================================================================
    // O(1) Decision Evaluation
    // ========================================================================

    /// Evaluate the network context against configured rules.
    ///
    /// Performs O(1) hash lookups against blocked criteria. Returns [`FilterVerdict::Block`]
    /// if any rule matches, otherwise [`FilterVerdict::Allow`].
    pub fn evaluate(&self, ctx: &NetworkFilterContext) -> FilterVerdict {
        if !self.is_enabled() {
            return FilterVerdict::Allow;
        }

        // 1. O(1) Check: Is calling PID blocked?
        if ctx.pid != 0 && self.blocked_pids.read().unwrap().contains(&ctx.pid) {
            return FilterVerdict::Block;
        }

        // 2. O(1) Check: Is (PID, Operation) blocked?
        if ctx.pid != 0
            && self
                .blocked_pid_ops
                .read()
                .unwrap()
                .contains(&(ctx.pid, ctx.operation))
        {
            return FilterVerdict::Block;
        }

        // 3. O(1) Checks on local address (source)
        if let Some(local) = ctx.local_addr {
            let endpoints = self.blocked_endpoints.read().unwrap();
            if endpoints.contains(&local) {
                return FilterVerdict::Block;
            }
            drop(endpoints);

            let ips = self.blocked_ips.read().unwrap();
            if ips.contains(&local.ip) {
                return FilterVerdict::Block;
            }
            drop(ips);

            let ports = self.blocked_ports.read().unwrap();
            if ports.contains(&local.port) {
                return FilterVerdict::Block;
            }
            drop(ports);

            let ops = self.blocked_ops.read().unwrap();
            if ops.contains(&(ctx.operation, local)) {
                return FilterVerdict::Block;
            }
            drop(ops);
        }

        // 4. O(1) Checks on remote address (destination)
        if let Some(remote) = ctx.remote_addr {
            let endpoints = self.blocked_endpoints.read().unwrap();
            if endpoints.contains(&remote) {
                return FilterVerdict::Block;
            }
            drop(endpoints);

            let ips = self.blocked_ips.read().unwrap();
            if ips.contains(&remote.ip) {
                return FilterVerdict::Block;
            }
            drop(ips);

            let ports = self.blocked_ports.read().unwrap();
            if ports.contains(&remote.port) {
                return FilterVerdict::Block;
            }
            drop(ports);

            let ops = self.blocked_ops.read().unwrap();
            if ops.contains(&(ctx.operation, remote)) {
                return FilterVerdict::Block;
            }
            drop(ops);
        }

        // 5. Evaluate custom policy hook if registered
        if let Some(ref hook) = *self.custom_hook.read().unwrap() {
            if hook(ctx) == FilterVerdict::Block {
                return FilterVerdict::Block;
            }
        }

        FilterVerdict::Allow
    }
}

impl Default for NetworkFilterHook {
    fn default() -> Self {
        Self::new()
    }
}

/// Global system-wide Network Filter Hook instance.
static GLOBAL_NETWORK_FILTER: OnceLock<NetworkFilterHook> = OnceLock::new();

/// Access the global singleton Network Filter Hook.
#[inline]
pub fn global_network_filter() -> &'static NetworkFilterHook {
    GLOBAL_NETWORK_FILTER.get_or_init(NetworkFilterHook::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_context_accessors() {
        let local = SocketAddress::new(127, 0, 0, 1, 8080);
        let remote = SocketAddress::new(192, 168, 1, 100, 443);
        let ctx = NetworkFilterContext::new(
            1337,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            Some(local),
            Some(remote),
        );

        assert_eq!(ctx.pid, 1337);
        assert_eq!(ctx.protocol, SocketProtocol::Tcp);
        assert_eq!(ctx.operation, NetworkFilterOp::Connect);
        assert_eq!(ctx.src_ip(), Some([127, 0, 0, 1]));
        assert_eq!(ctx.src_port(), Some(8080));
        assert_eq!(ctx.dst_ip(), Some([192, 168, 1, 100]));
        assert_eq!(ctx.dst_port(), Some(443));
    }

    #[test]
    fn test_filter_o1_pid_blocking() {
        let hook = NetworkFilterHook::new();
        let target = SocketAddress::new(10, 0, 0, 1, 80);
        let ctx_allowed = NetworkFilterContext::new(
            100,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(target),
        );
        let ctx_blocked = NetworkFilterContext::new(
            200,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(target),
        );

        hook.block_pid(200);

        assert_eq!(hook.evaluate(&ctx_allowed), FilterVerdict::Allow);
        assert_eq!(hook.evaluate(&ctx_blocked), FilterVerdict::Block);

        hook.unblock_pid(200);
        assert_eq!(hook.evaluate(&ctx_blocked), FilterVerdict::Allow);
    }

    #[test]
    fn test_filter_o1_port_blocking() {
        let hook = NetworkFilterHook::new();
        let web_dest = SocketAddress::new(10, 0, 0, 1, 80);
        let ssh_dest = SocketAddress::new(10, 0, 0, 1, 22);

        let ctx_web = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(web_dest),
        );
        let ctx_ssh = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(ssh_dest),
        );

        hook.block_port(22);

        assert_eq!(hook.evaluate(&ctx_web), FilterVerdict::Allow);
        assert_eq!(hook.evaluate(&ctx_ssh), FilterVerdict::Block);
    }

    #[test]
    fn test_filter_o1_ip_blocking() {
        let hook = NetworkFilterHook::new();
        let bad_ip = [198, 51, 100, 1];
        let good_ip = [198, 51, 100, 2];

        let ctx_bad = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(SocketAddress::new(bad_ip[0], bad_ip[1], bad_ip[2], bad_ip[3], 443)),
        );
        let ctx_good = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(SocketAddress::new(good_ip[0], good_ip[1], good_ip[2], good_ip[3], 443)),
        );

        hook.block_ip(bad_ip);

        assert_eq!(hook.evaluate(&ctx_good), FilterVerdict::Allow);
        assert_eq!(hook.evaluate(&ctx_bad), FilterVerdict::Block);
    }

    #[test]
    fn test_filter_o1_endpoint_and_operation_blocking() {
        let hook = NetworkFilterHook::new();
        let target = SocketAddress::new(192, 168, 1, 50, 9000);

        let ctx_bind = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Bind,
            Some(target),
            None,
        );
        let ctx_connect = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(target),
        );

        // Block Bind only on target
        hook.block_operation(NetworkFilterOp::Bind, target);

        assert_eq!(hook.evaluate(&ctx_bind), FilterVerdict::Block);
        assert_eq!(hook.evaluate(&ctx_connect), FilterVerdict::Allow);

        // Block entire endpoint
        hook.block_endpoint(target);
        assert_eq!(hook.evaluate(&ctx_connect), FilterVerdict::Block);
    }

    #[test]
    fn test_filter_custom_hook() {
        let hook = NetworkFilterHook::new();
        // Custom rule: Block all UDP traffic from PID 500
        hook.set_custom_hook(|ctx| {
            if ctx.pid == 500 && ctx.protocol == SocketProtocol::Udp {
                FilterVerdict::Block
            } else {
                FilterVerdict::Allow
            }
        });

        let ctx_udp = NetworkFilterContext::new(
            500,
            SocketProtocol::Udp,
            NetworkFilterOp::SendTo,
            None,
            Some(SocketAddress::loopback(5000)),
        );
        let ctx_tcp = NetworkFilterContext::new(
            500,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(SocketAddress::loopback(5000)),
        );

        assert_eq!(hook.evaluate(&ctx_udp), FilterVerdict::Block);
        assert_eq!(hook.evaluate(&ctx_tcp), FilterVerdict::Allow);
    }

    #[test]
    fn test_filter_master_toggle() {
        let hook = NetworkFilterHook::new();
        hook.block_port(80);

        let ctx = NetworkFilterContext::new(
            1,
            SocketProtocol::Tcp,
            NetworkFilterOp::Connect,
            None,
            Some(SocketAddress::loopback(80)),
        );

        assert_eq!(hook.evaluate(&ctx), FilterVerdict::Block);

        hook.set_enabled(false);
        assert_eq!(hook.evaluate(&ctx), FilterVerdict::Allow);

        hook.set_enabled(true);
        assert_eq!(hook.evaluate(&ctx), FilterVerdict::Block);
    }
}
