@0xf741b2ece88683fb;

using Rpc = import "rpc.capnp";

enum RegistryMessage {
    register @0;
    get @1;
}

struct Register {
    name @0 :Text;
    handle @1 :Rpc.HandleIndex;
}

struct RegisterResp {}

struct Get {
    name @0 :Text;

    mode :union {
        any :group {
            blocking @1 :Bool;
            # wait for something
        }
        stream :group {
            continue @2 :Bool;
            # just give known values or subscribe
        }
    }
}

struct GetResp {
    entries @0 :List(Rpc.HandleIndex);
    extra @1 :Rpc.HandleIndex;
    # a stream of values to subscribe to
}
