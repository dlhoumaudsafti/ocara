use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

/// Builtins du module DateTime
pub const DATETIME_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "DateTime_now",            params: &[],                   returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_fromTimestamp",  params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_year",           params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_month",          params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_day",            params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_hour",           params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_minute",         params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_second",         params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_format",         params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("DateTime") },
    BuiltinDesc { name: "DateTime_parse",          params: &[clt::I64],           returns: Some(clt::I64), module: Some("DateTime") },
];

/// Builtins du module Date
pub const DATE_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Date_today",          params: &[],                   returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_fromTimestamp",  params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_year",           params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_month",          params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_day",            params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_dayOfWeek",      params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_isLeapYear",     params: &[clt::I64],           returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_daysInMonth",    params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_addDays",        params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Date") },
    BuiltinDesc { name: "Date_diffDays",       params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Date") },
];

/// Builtins du module Time
pub const TIME_BUILTINS: &[BuiltinDesc] = &[
    BuiltinDesc { name: "Time_now",           params: &[],                   returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_fromTimestamp", params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_hour",          params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_minute",        params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_second",        params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_fromSeconds",   params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_toSeconds",     params: &[clt::I64],           returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_addSeconds",    params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Time") },
    BuiltinDesc { name: "Time_diffSeconds",   params: &[clt::I64, clt::I64], returns: Some(clt::I64), module: Some("Time") },
];
