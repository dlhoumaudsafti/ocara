use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module YAML
pub const YAML_BUILTINS: &[BuiltinDesc] = &[
    // YAML::encode(data) → string
    BuiltinDesc { 
        name: "YAML_encode", 
        params: &[clt::I64],      // data:mixed
        returns: Some(clt::I64),  // → string
        module: Some("YAML") 
    },
    
    // YAML::decode(yaml) → mixed
    BuiltinDesc { 
        name: "YAML_decode", 
        params: &[clt::I64],      // yaml:string
        returns: Some(clt::I64),  // → mixed
        module: Some("YAML") 
    },
    
    // YAML::parse(yaml) → mixed (alias)
    BuiltinDesc { 
        name: "YAML_parse", 
        params: &[clt::I64],      // yaml:string
        returns: Some(clt::I64),  // → mixed
        module: Some("YAML") 
    },
];
