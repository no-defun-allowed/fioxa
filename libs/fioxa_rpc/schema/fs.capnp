@0xa1515e7204ddd567;

using Rpc = import "rpc.capnp";

enum FolderMessage {
    getChildren @0;
    open @1;
    describe @2;
}

enum FileMessage {
    size @0;
    read @1;
    write @2;
}

enum FileType {
    none @0;
    file @1;
    folder @2;
}

struct FolderGetChildren {}

struct FolderGotChildren {
    struct Entry {
        name @0 :Text;
        type @1 :FileType;
    }
    entries @0 :List(Entry);
}

struct FolderOpen {
    name @0 :Text;
}

struct FolderOpened {
    type @0 :FileType;
    capability @1 :Rpc.HandleIndex;
}

struct FolderDescribe {}

struct FolderInfo {
    name @0 :Text;
}

struct FileSize {}

struct FileSizeRead {
    size @0 :UInt64;
}

struct FileRead {
    offset @0 :UInt64;
    len @1 :UInt32;
}

struct FileData {
    data @0 :Data;
}

struct FileWrite {
    offset @0 :UInt64;
    data @1 :Data;
}

struct FileWriteResp {}

