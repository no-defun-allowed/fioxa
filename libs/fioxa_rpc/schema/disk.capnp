@0xa1b5ee225d961337;

using Rpc = import "rpc.capnp";

enum DiskMessage {
    read @0;
    identify @1;
}

struct Read {
    sector @0 :UInt64;
    count @1 :UInt32;
    handle @2 :Rpc.HandleIndex;
}

struct ReadResp {
    data @0 :Data;
}

struct Identify {}

