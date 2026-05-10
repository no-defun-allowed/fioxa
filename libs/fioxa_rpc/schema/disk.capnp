@0xa1b5ee225d961337;

using Rpc = import "rpc.capnp";

enum DiskMessage {
    read @0;
    identify @1;
    write @2;
}

struct Read {
    sector @0 :UInt64;
    count @1 :UInt32;
}

struct ReadResp {
    data @0 :Data;
}

struct Identify {}

struct Write {
    sector @0 :UInt64;
    data @1 :Data;
}

struct WriteResp {}

