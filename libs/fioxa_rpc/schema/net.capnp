@0x92b7fb8c5eb7af9e;

using Rpc = import "rpc.capnp";

enum EthMessage {
    getMac @0;
    sendPacket @1;
    listenToPackets @2;
}

struct EthGetMac {}

struct MacAddr {
    val @0 :UInt64;
}

struct EthSendPacket {
    packet @0 :Data;
}

struct Empty {}

struct EthListenToPackets {
    channel @0 :Rpc.HandleIndex;
}


enum NetMessage {
    arpRequest @0;
}

struct ArpRequest {
    ip @0 :UInt32;
}

struct ArpReponse {
    enum ArpSuccess {
        success @0;
        notSameSubnet @1;
        unknown @2;
    }
    success @0 :ArpSuccess;
    mac @1 :UInt64;
    # only meaningful if success
}

