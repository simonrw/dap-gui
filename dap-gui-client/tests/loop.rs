use std::net::TcpStream;
// Loop 
// Initialize
// Launch
// Set function breakpoints
// Continue
#[test]
fn test_loop() {
    // TODO: automatic setup for running the server
    // for now assume the server is running
    let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
    let client = dap_gui_client::Client::new(stream);
}
