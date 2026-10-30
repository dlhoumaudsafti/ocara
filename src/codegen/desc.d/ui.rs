// src/codegen/desc.d/ui.rs — signatures Cranelift pour builtin UI (backend
// desktop : runtime_tauri/src/ui ; Android : runtime_android/ui) et UIEvent
// (runtime_ui/src/event.rs). `this` : l'objet UI ou la valeur UIEvent.

use crate::codegen::runtime::BuiltinDesc;
use cranelift_codegen::ir::types as clt;

const fn ui(name: &'static str, params: &'static [clt::Type], returns: Option<clt::Type>) -> BuiltinDesc {
    BuiltinDesc { name, params, returns, module: Some("UI") }
}

const fn event(name: &'static str, returns: clt::Type) -> BuiltinDesc {
    BuiltinDesc { name, params: &[I], returns: Some(returns), module: Some("UIEvent") }
}

const fn component(name: &'static str, params: &'static [clt::Type], returns: Option<clt::Type>) -> BuiltinDesc {
    BuiltinDesc { name, params, returns, module: Some("UIComponent") }
}

const I: clt::Type = clt::I64;

pub const UI_BUILTINS: &[BuiltinDesc] = &[
    ui("UI_init",     &[I, I, I],    None), // this, arbre JSON (littéral, src/core/ui_file.rs), vars
    ui("UI_setVars",  &[I, I],       None),
    ui("UI_sizeClass", &[I],         Some(I)),
    ui("UI_has",      &[I, I],       Some(I)),
    ui("UI_component", &[I, I],      Some(I)),
    ui("UI_on",       &[I, I, I],    None), // this, action, gestionnaire
    ui("UI_get",      &[I, I, I],    Some(I)),
    ui("UI_set",      &[I, I, I, I], None),
    ui("UI_bind",     &[I, I, I, I, I], None), // this, racine, objet, chemins de l'accesseur (JSON), accesseur
    ui("UI_unbind",   &[I, I],       None),
    ui("UI_refresh",  &[I],          None),
    ui("UI_setItems", &[I, I, I],    None),
    ui("UI_navigate", &[I, I],       None),
    ui("UI_back",     &[I],          None),
    ui("UI_run",      &[I],          None),
    ui("UI_close",    &[I],          None),
    ui("UI_toast",    &[I, I],       None),
    ui("UI_alert",    &[I, I, I],    None),
    ui("UI_confirm",  &[I, I, I],    Some(I)),
    ui("UI_pickFile", &[I, I],       Some(I)),
    ui("UI_saveFile", &[I, I],       Some(I)),
    ui("UI_notify",   &[I, I, I],    None),
    ui("UI_tauri",       &[I],       Some(I)),
    ui("UI_postMessage", &[I, I, I], None),
    ui("UI_eval",        &[I, I, I], Some(I)),
    ui("UI_webBack",     &[I, I],    None),
    ui("UI_webForward",  &[I, I],    None),
    ui("UI_reload",      &[I, I],    None),
    event("UIEvent_action", I),
    event("UIEvent_id",     I),
    event("UIEvent_value",  I),
    event("UIEvent_index",  I),
    event("UIEvent_instance", I),
    component("UIComponent_on",        &[I, I, I], None), // this, action, gestionnaire
    component("UIComponent_name",      &[I],       Some(I)),
    component("UIComponent_instances", &[I],       Some(I)),
    component("UIComponent_id",        &[I, I, I], Some(I)),
];
