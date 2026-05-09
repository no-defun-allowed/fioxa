@0xa239d749673529f1;

using Rpc = import "rpc.capnp";

enum ElfMessage {
    spawn @0;
}

struct Spawn {
    file @0 :Rpc.HandleIndex;
    initialRefs @1 :List(Rpc.HandleIndex);
}

struct Spawned {
    handle @0 :Rpc.HandleIndex;
}

