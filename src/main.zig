const std = @import("std");

const requests = @import("./requests");

const Client = struct {
    stream: std.net.Stream,

    fn new(stream: std.net.Stream) !Client {
        return Client{
            .stream = stream,
        };
    }

    fn close(self: Client) void {
        self.stream.close();
    }

    fn send(self: Client, body: requests.RequestBody) !void {
        _ = body;
        _ = self;
    }
};

pub fn main() !void {
    const address = try std.net.Address.parseIp4("127.0.0.1", 5678);
    const s = try std.net.tcpConnectToAddress(address);
    const client = try Client.new(s);
    defer client.close();

    std.debug.print("Client created", .{});
}
