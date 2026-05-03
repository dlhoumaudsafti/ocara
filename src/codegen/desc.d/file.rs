use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module File
pub const FILE_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "File_read",       params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
    BuiltinDesc { name: "File_readBytes",  params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
    BuiltinDesc { name: "File_write",      params: &[clt::I64, clt::I64],     returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_writeBytes", params: &[clt::I64, clt::I64],     returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_append",     params: &[clt::I64, clt::I64],     returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_exists",     params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
    BuiltinDesc { name: "File_size",       params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
    BuiltinDesc { name: "File_extension",  params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
    BuiltinDesc { name: "File_remove",     params: &[clt::I64],               returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_copy",       params: &[clt::I64, clt::I64],     returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_move",       params: &[clt::I64, clt::I64],     returns: None,           module: Some("File") },
    BuiltinDesc { name: "File_infos",      params: &[clt::I64],               returns: Some(clt::I64), module: Some("File") },
];

/// Builtins du module Directory
pub const DIRECTORY_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Directory_create",           params: &[clt::I64],           returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_createRecursive",  params: &[clt::I64],           returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_remove",           params: &[clt::I64],           returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_removeRecursive",  params: &[clt::I64],           returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_list",             params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
    BuiltinDesc { name: "Directory_listFiles",        params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
    BuiltinDesc { name: "Directory_listDirs",         params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
    BuiltinDesc { name: "Directory_exists",           params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
    BuiltinDesc { name: "Directory_count",            params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
    BuiltinDesc { name: "Directory_copy",             params: &[clt::I64, clt::I64], returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_move",             params: &[clt::I64, clt::I64], returns: None,           module: Some("Directory") },
    BuiltinDesc { name: "Directory_infos",            params: &[clt::I64],           returns: Some(clt::I64), module: Some("Directory") },
];
