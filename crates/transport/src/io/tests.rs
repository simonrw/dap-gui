//! Tests for transport implementations

use super::*;

#[test]
fn test_tcp_transport_implements_trait() {
    // Compile-time verification that TcpTransport implements DapTransport
    fn _assert_impl<T: DapTransport>() {}
    _assert_impl::<TcpTransport>();
}

#[test]
fn test_memory_transport_implements_trait() {
    // Compile-time verification that InMemoryTransport implements DapTransport
    fn _assert_impl<T: DapTransport>() {}
    _assert_impl::<InMemoryTransport>();
}
