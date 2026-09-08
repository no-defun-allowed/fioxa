@0xd091bb5c22ae9ef6;

using Rpc = import "rpc.capnp";

enum FramebufferMessage {
  getInfo @0;
  acquire @1;
  release @2;
}

struct FramebufferGetInfo {}

struct FramebufferInfo {
  capability @0 :Rpc.HandleIndex;
  offset @1 :UInt64;
  size @2 :UInt64;
  width @3 :UInt16;
  height @4 :UInt16;
  stride @5 :UInt16;
}

struct FramebufferAcquire {}
struct FramebufferRelease {}

struct FramebufferChanged {}