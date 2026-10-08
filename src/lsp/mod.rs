//! Serveur de langage (`ocara --lsp`, JSON-RPC sur stdin/stdout) : réutilise
//! l'analyse du compilateur (`core::analysis`) — voir
//! docs/roadmap.d/tooling-language-server.md.

mod builtin_docs;
mod decls;
mod features;
mod position;
mod workspace;

use std::path::{Path, PathBuf};

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument, Notification as _,
    PublishDiagnostics,
};
use lsp_types::request::{DocumentSymbolRequest, GotoDefinition, HoverRequest, Request as _};
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
        ..Default::default()
    };
    let Ok(init) = connection.initialize(serde_json::to_value(capabilities).unwrap_or(Value::Null)) else { return };
    let params: InitializeParams = serde_json::from_value(init).unwrap_or_default();
    let roots = params.workspace_folders.unwrap_or_default().iter()
        .filter_map(|f| to_path(&f.uri))
        .collect();
    let mut ws = Workspace::new(roots);

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req).unwrap_or(true) {
                    break;
                }
                let response = handle_request(&ws, req);
                let _ = connection.sender.send(Message::Response(response));
            }
            Message::Notification(note) => handle_notification(&connection, &mut ws, note),
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

fn handle_request(ws: &Workspace, req: Request) -> Response {
    let result = match req.method.as_str() {
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
        DocumentSymbolRequest::METHOD => serde_json::from_value::<lsp_types::DocumentSymbolParams>(req.params)
            .ok()
            .and_then(|p| to_path(&p.text_document.uri))
            .map(|path| serde_json::to_value(DocumentSymbolResponse::Nested(features::document_symbols(ws, &path))).unwrap_or(Value::Null)),
        _ => return Response::new_err(req.id, lsp_server::ErrorCode::MethodNotFound as i32, format!("méthode non prise en charge : {}", req.method)),
    };
    Response::new_ok(req.id, result.unwrap_or(Value::Null))
}

fn handle_notification(connection: &Connection, ws: &mut Workspace, note: Notification) {
    match note.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(note.params) else { return };
            let Some(path) = to_path(&p.text_document.uri) else { return };
            ws.open(path.clone(), p.text_document.text);
            refresh(connection, ws, &path);
        }
        DidChangeTextDocument::METHOD => {
            let Ok(p) = serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(note.params) else { return };
            let Some(path) = to_path(&p.text_document.uri) else { return };
            let Some(change) = p.content_changes.into_iter().last() else { return };
            ws.open(path.clone(), change.text);
            refresh(connection, ws, &path);
        }
        DidSaveTextDocument::METHOD => {
            // Un fichier enregistré peut changer l'analyse des autres
            // documents ouverts qui l'importent.
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
