@0xaf2e99c022b03d53;

struct Call {
    interfaceId @0 :UInt64;

    methodId @1 :UInt16;

    payload @2 :AnyPointer;
}

struct HandleIndex {
    index @0 :UInt8;
}
