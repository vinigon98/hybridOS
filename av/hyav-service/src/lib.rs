//! hyav-service: HybridOS Anti-Virus Host Security Service & Network Policy Engine
//!
//! Provides firewall policy evaluation across Domain, Private, and Public network profiles,
//! active threat response actions (TCP RST synthesis, temporary IP banning with TTL),
//! and audit logging via JSON-Lines telemetry (Windows Event IDs 1117 / 1118).

#![forbid(unsafe_code)]

pub mod net_policy;

pub use net_policy::*;
