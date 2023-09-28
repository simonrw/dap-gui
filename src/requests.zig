const InitializeBody = struct {
    adapterId: []const u8,
};

const RequestBody = union(enum) {
    Initialize: InitializeBody,
};
