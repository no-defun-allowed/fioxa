@0xf2a8242a317c8cae;

using Rpc = import "rpc.capnp";

enum InterruptMessage {
    subscribe @0;
}

enum Vector {
    keyboard @0;
    mouse @1;
    pci @2;
    com1 @3;
}

struct Subscribe {
    vector @0 :Vector;
}

struct SubscribeResp {
    handle @0 :Rpc.HandleIndex;
}