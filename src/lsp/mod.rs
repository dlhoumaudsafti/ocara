//! Serveur de langage (`ocara --lsp`, JSON-RPC sur stdin/stdout) : réutilise
//! l'analyse du compilateur (`core::analysis`) — voir
//! docs/roadmap.d/tooling-language-server.md.

mod auto_import;
mod builtin_docs;
mod callsite;
mod completion;
mod decls;
mod features;
mod keywords;
pub(crate) mod layers;
mod links;
mod navigation;
mod position;
mod project;
mod semantic;
mod signature;
mod workflow;
mod workflow_calls;
mod workflow_code;
mod workspace;

use std::path::{Path, PathBuf};

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{CodeActionRequest, CodeLensRequest, Completion, DocumentLinkRequest, DocumentSymbolRequest, GotoDefinition, HoverRequest, References, Rename, Request as _, SemanticTokensFullRequest, SignatureHelpRequest};
use lsp_types::{
    DocumentSymbolResponse, GotoDefinitionResponse, InitializeParams, OneOf, PublishDiagnosticsParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Url,
};
use serde_json::Value;

use workspace::Workspace;

pub fn run() {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::FULL),
            save: Some(TextDocumentSyncSaveOptions::Supported(true)),
            ..Default::default()
        })),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            trigger_characters: Some(vec![".".into(), ":".into(), "(".into(), ",".into(), " ".into()]),
            ..Default::default()
        }),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Left(true)),
        code_lens_provider: Some(lsp_types::CodeLensOptions { resolve_provider: Some(false) }),
        semantic_tokens_provider: Some(lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(lsp_types::SemanticTokensOptions {
            legend: semantic::legend(),
            full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
            ..Default::default()
        })),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(lsp_types::CodeActionOptions {
            code_action_kinds: Some(vec![lsp_types::CodeActionKind::QUICKFIX, lsp_types::CodeActionKind::new(auto_import::SOURCE_KIND)]),
            ..Default::default()
        })),
        document_link_provider: Some(lsp_types::DocumentLinkOptions { resolve_provider: Some(false), work_done_progress_options: Default::default() }),
        signature_help_provider: Some(lsp_types::SignatureHelpOptions {
            trigger_characters: Some(vec!["(".into(), ",".into()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let Ok(init) = connection.initialize(serde_json::to_value(capabilities).unwrap_or(Value::Null)) else { return };
    let params: InitializeParams = serde_json::from_value(init).unwrap_or_default();
    apply_build_settings(params.initialization_options.as_ref());
    let roots = params.workspace_folders.unwrap_or_default().iter()
        .filter_map(|f| to_path(&f.uri))
        .collect();
    let mut ws = Workspace::new(roots);
    let mut index = project::ProjectIndex::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req).unwrap_or(true) {
                    break;
                }
                let response = handle_request(&ws, &mut index, req);
                let _ = connection.sender.send(Message::Response(response));
            }
            Message::Notification(note) => handle_notification(&connection, &mut ws, &mut index, note),
            Message::Response(_) => {}
        }
    }
    drop(connection);
    let _ = io_threads.join();
}

fn to_path(url: &Url) -> Option<PathBuf> {
    let path = url.to_file_path().ok()?;
    Some(path.canonicalize().unwrap_or(path))
}

fn handle_request(ws: &Workspace, index: &mut project::ProjectIndex, req: Request) -> Response {
    let result = match req.method.as_str() {
        References::METHOD => serde_json::from_value::<lsp_types::ReferenceParams>(req.params)
            .ok()
            .and_then(|p| {
                index.ensure(ws);
                let pos = p.text_document_position;
                Some(navigation::references(ws, index, &to_path(&pos.text_document.uri)?, &pos.position, p.context.include_declaration))
            })
            .map(|l| serde_json::to_value(l).unwrap_or(Value::Null)),
        Rename::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::RenameParams>(req.params) else {
                return Response::new_err(req.id, lsp_server::ErrorCode::InvalidParams as i32, "paramètres invalides".into());
            };
            index.ensure(ws);
            let pos = p.text_document_position;
            let Some(path) = to_path(&pos.text_document.uri) else {
                return Response::new_err(req.id, lsp_server::ErrorCode::InvalidParams as i32, "document inconnu".into());
            };
            match navigation::rename(ws, index, &path, &pos.position, &p.new_name) {
                Ok(edit) => Some(serde_json::to_value(edit).unwrap_or(Value::Null)),
                Err(message) => return Response::new_err(req.id, lsp_server::ErrorCode::RequestFailed as i32, message),
            }
        }
        CodeLensRequest::METHOD => serde_json::from_value::<lsp_types::CodeLensParams>(req.params)
            .ok()
            .and_then(|p| {
                index.ensure(ws);
                Some(navigation::code_lenses(ws, index, &to_path(&p.text_document.uri)?))
            })
            .map(|l| serde_json::to_value(l).unwrap_or(Value::Null)),
        HoverRequest::METHOD => serde_json::from_value::<lsp_types::HoverParams>(req.params)
            .ok()
            .and_then(|p| {
                let pos = p.text_document_position_params;
                features::hover(ws, &to_path(&pos.text_document.uri)?, &pos.position)
            })
            .map(|h| serde_json::to_value(h).unwrap_or(Value::Null)),
        GotoDefinition::METHOD => serde_json::from_value::<lsp_types::GotoDefinitionParams>(req.params)
            .ok()
            .and_then(|p| {
                let pos = p.text_document_position_params;
                features::definition(ws, &to_path(&pos.text_document.uri)?, &pos.position)
            })
            .map(|l| serde_json::to_value(GotoDefinitionResponse::Scalar(l)).unwrap_or(Value::Null)),
        SemanticTokensFullRequest::METHOD => serde_json::from_value::<lsp_types::SemanticTokensParams>(req.params)
            .ok()
            .and_then(|p| to_path(&p.text_document.uri))
            .and_then(|path| {
                let layers = layers::config(ws, &path).map(|c| layers::LayerSyntax::from_config(&c));
                Some(semantic::tokens(&ws.text(&path)?, layers.as_ref()))
            })
            .map(|tokens| serde_json::to_value(lsp_types::SemanticTokensResult::Tokens(tokens)).unwrap_or(Value::Null)),
        CodeActionRequest::METHOD => serde_json::from_value::<lsp_types::CodeActionParams>(req.params)
            .ok()
            .and_then(|p| Some(auto_import::code_actions(ws, &to_path(&p.text_document.uri)?, &p)))
            .map(|a| serde_json::to_value(a).unwrap_or(Value::Null)),
        DocumentLinkRequest::METHOD => serde_json::from_value::<lsp_types::DocumentLinkParams>(req.params)
            .ok()
            .and_then(|p| to_path(&p.text_document.uri))
            .map(|path| serde_json::to_value(links::document_links(ws, &path)).unwrap_or(Value::Null)),
        DocumentSymbolRequest::METHOD => serde_json::from_value::<lsp_types::DocumentSymbolParams>(req.params)
            .ok()
            .and_then(|p| to_path(&p.text_document.uri))
            .map(|path| serde_json::to_value(DocumentSymbolResponse::Nested(features::document_symbols(ws, &path))).unwrap_or(Value::Null)),
        Completion::METHOD => serde_json::from_value::<lsp_types::CompletionParams>(req.params)
            .ok()
            .and_then(|p| {
                let pos = p.text_document_position;
                Some(completion::completion(ws, &to_path(&pos.text_document.uri)?, &pos.position))
            })
            .map(|items| serde_json::to_value(lsp_types::CompletionResponse::Array(items)).unwrap_or(Value::Null)),
        SignatureHelpRequest::METHOD => serde_json::from_value::<lsp_types::SignatureHelpParams>(req.params)
            .ok()
            .and_then(|p| {
                let pos = p.text_document_position_params;
                signature::signature_help(ws, &to_path(&pos.text_document.uri)?, &pos.position)
            })
            .map(|h| serde_json::to_value(h).unwrap_or(Value::Null)),
        workflow::METHOD => serde_json::from_value::<lsp_types::TextDocumentIdentifier>(req.params.get("textDocument").cloned().unwrap_or(Value::Null))
            .ok()
            .and_then(|d| to_path(&d.uri))
            .map(|path| workflow::workflow(ws, &path)),
        _ => return Response::new_err(req.id, lsp_server::ErrorCode::MethodNotFound as i32, format!("méthode non prise en charge : {}", req.method)),
    };
    // Noms internes des types homonymes (`Format__ns__…`) jamais montrés.
    let result = result.map(|v| {
        let text = crate::core::analysis::homonyms::display(&v.to_string());
        serde_json::from_str(&text).unwrap_or(v)
    });
    Response::new_ok(req.id, result.unwrap_or(Value::Null))
}

fn handle_notification(connection: &Connection, ws: &mut Workspace, index: &mut project::ProjectIndex, note: Notification) {
    match note.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(note.params) else { return };
            let Some(path) = to_path(&p.text_document.uri) else { return };
            ws.open(path.clone(), p.text_document.text);
            index.invalidate(&path);
            refresh(connection, ws, &path);
        }
        DidChangeTextDocument::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(note.params) else { return };
            let Some(path) = to_path(&p.text_document.uri) else { return };
            let Some(change) = p.content_changes.into_iter().last() else { return };
            ws.open(path.clone(), change.text);
            index.invalidate(&path);
            refresh(connection, ws, &path);
        }
        DidSaveTextDocument::METHOD => {
            // Un fichier enregistré peut changer l'analyse des autres
            // documents ouverts qui l'importent.
            for path in ws.open_paths() {
                refresh(connection, ws, &path);
            }
        }
        // Fichier `.oc` créé, modifié ou supprimé hors de l'éditeur : index
        // mis à jour, documents ouverts réanalysés (ils peuvent l'importer).
        DidChangeWatchedFiles::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidChangeWatchedFilesParams>(note.params) else { return };
            for change in &p.changes {
                if let Ok(path) = change.uri.to_file_path() {
                    index.invalidate(&path.canonicalize().unwrap_or(path));
                }
            }
            for path in ws.open_paths() {
                refresh(connection, ws, &path);
            }
        }
        // Réglages `ocara.os`/`ocara.arch`/`ocara.build` : nouvelle configuration active,
        // documents ouverts réanalysés (variantes `when` estompées).
        DidChangeConfiguration::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidChangeConfigurationParams>(note.params) else { return };
            apply_build_settings(Some(&p.settings));
            for path in ws.open_paths() {
                refresh(connection, ws, &path);
            }
        }
        DidCloseTextDocument::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidCloseTextDocumentParams>(note.params) else { return };
            let Some(path) = to_path(&p.text_document.uri) else { return };
            ws.close(&path);
            publish(connection, &path, Vec::new());
        }
        _ => {}
    }
}

/// Configuration active de l'éditeur : `{ "os": …, "arch": …, "build": … }`
/// (absents : valeurs de l'hôte, mode debug).
fn apply_build_settings(settings: Option<&Value>) {
    let mut config = crate::parsing::build::BuildConfig::for_target(None, false);
    for (key, name) in [("os", "OS"), ("arch", "ARCH"), ("build", "BUILD")] {
        if let Some(value) = settings.and_then(|s| s.get(key)).and_then(Value::as_str).filter(|v| !v.is_empty()) {
            config.values.insert(name.to_string(), value.to_string());
        }
    }
    crate::parsing::build::set_current(config);
}

fn refresh(connection: &Connection, ws: &mut Workspace, path: &Path) {
    ws.analyze(path);
    let Some(analysis) = ws.analysis(path) else { return };
    let diagnostics = features::diagnostics(ws, path, analysis);
    publish(connection, path, diagnostics);
}

fn publish(connection: &Connection, path: &Path, diagnostics: Vec<lsp_types::Diagnostic>) {
    let params = PublishDiagnosticsParams { uri: features::to_url(path), diagnostics, version: None };
    let note = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
    let _ = connection.sender.send(Message::Notification(note));
}
