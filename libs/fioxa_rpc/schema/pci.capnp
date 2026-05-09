@0xea6588235deb829f;

enum PCIMessage {
    read @0;
    write @1;
}

enum Size {
    u8 @0;
    u16 @1;
    u32 @2;
}

struct Read {
    offset @0 :UInt32;
    size @1 :Size;
}

struct ReadRes {
    val @0 :UInt32;
    # is expected to be 0 extended
}

struct Write {
    offset @0 :UInt32;
    size @1 :Size;
    val @2 :UInt32;
    # is expected to be 0 extended
}

struct WriteRes {}
