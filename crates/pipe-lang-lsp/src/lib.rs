use std::collections::HashMap;

use ast::span::Span;
use bumpalo::Bump;
use diagnostics::CompilerError;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidOpenTextDocumentParams, Hover,
    HoverContents, HoverParams, InitializeParams, InitializeResult, MarkedString, Position, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};
use typechecker::TypeError;

#[derive(Debug, Clone, Default)]
struct DocumentState {
    source: String,
    type_map: HashMap<Span, String>,
}

/// Language-server backend for pipe-lang editor tooling.
pub struct Backend {
    client: Client,
    documents: tokio::sync::RwLock<HashMap<Url, DocumentState>>,
}

impl Backend {
    /// Creates a new language-server backend.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    async fn analyze_and_publish(&self, uri: Url, source: String) {
        let (type_map, diagnostics) = analyze_source(&source);
        self.documents
            .write()
            .await
            .insert(uri.clone(), DocumentState { source, type_map });
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn hover_for_params(&self, params: HoverParams) -> Option<Hover> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        let document = documents.get(&uri)?;
        hover_for_position(document, position)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: None,
        })
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.analyze_and_publish(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze_and_publish(params.text_document.uri, change.text)
                .await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(self.hover_for_params(params).await)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

fn analyze_source(source: &str) -> (HashMap<Span, String>, Vec<Diagnostic>) {
    let arena = Bump::new();
    let program = match parser::parse(source, &arena) {
        Ok(program) => program,
        Err(err) => {
            return (
                HashMap::new(),
                vec![compiler_error_to_lsp(source, CompilerError::from(err))],
            );
        }
    };

    match typechecker::typecheck(&program) {
        Ok(typed) => (
            typed
                .type_map
                .into_iter()
                .map(|(span, ty)| (span, ty.to_string()))
                .collect(),
            Vec::new(),
        ),
        Err(errors) => (
            HashMap::new(),
            errors
                .into_iter()
                .map(compiler_error_from_type_error)
                .map(|err| compiler_error_to_lsp(source, err))
                .collect(),
        ),
    }
}

fn compiler_error_from_type_error(error: TypeError) -> CompilerError {
    match error {
        TypeError::UnificationFailed {
            expected,
            got,
            span,
        } => CompilerError::type_error(
            span,
            format!("type mismatch: expected {expected}, got {got}"),
        ),
        TypeError::UnboundVariable { name, span } => {
            CompilerError::type_error(span, format!("unbound variable `{name}`"))
        }
        TypeError::ArityMismatch {
            expected,
            got,
            span,
        } => CompilerError::type_error(
            span,
            format!("arity mismatch: expected {expected} arguments, got {got}"),
        ),
        TypeError::InfiniteType { var, ty, span } => {
            CompilerError::type_error(span, format!("infinite type: {var} occurs in {ty}"))
        }
        TypeError::AnnotationConflict {
            annotation,
            inferred,
            span,
        } => {
            let msg = format!(
                "type annotation conflict: annotation says {annotation}, inferred {inferred}"
            );
            CompilerError::type_error(span, msg)
        }
        TypeError::NonExhaustiveMatch { span } => {
            CompilerError::type_error(span, "non-exhaustive match")
        }
        TypeError::FieldNotFound { field, span } => {
            CompilerError::type_error(span, format!("field `{field}` not found on record"))
        }
        TypeError::NumericOverflow { ty, span } => {
            CompilerError::type_error(span, format!("numeric literal overflows type `{ty}`"))
        }
    }
}

fn compiler_error_to_lsp(source: &str, error: CompilerError) -> Diagnostic {
    Diagnostic {
        range: error.span().map_or_else(
            || Range::new(Position::new(0, 0), Position::new(0, 0)),
            |span| span_to_range(source, span),
        ),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("pipe-lang".to_string()),
        message: error.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

fn hover_for_position(document: &DocumentState, position: Position) -> Option<Hover> {
    let offset = position_to_byte_offset(&document.source, position)?;
    let ty = document
        .type_map
        .iter()
        .filter(|(span, _)| {
            if span.is_empty() {
                span.start == offset
            } else {
                span.start <= offset && offset < span.end
            }
        })
        .min_by_key(|(span, _)| span.len())
        .map(|(_, ty)| ty)?;

    Some(Hover {
        contents: HoverContents::Scalar(MarkedString::String(format!("type: {ty}"))),
        range: None,
    })
}

fn span_to_range(source: &str, span: Span) -> Range {
    Range::new(
        byte_offset_to_position(source, span.start),
        byte_offset_to_position(source, span.end),
    )
}

fn byte_offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0_u32;
    let mut character = 0_u32;

    for (byte_idx, ch) in source.char_indices() {
        if byte_idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    Position::new(line, character)
}

fn position_to_byte_offset(source: &str, target: Position) -> Option<usize> {
    let mut line = 0_u32;
    let mut character = 0_u32;

    for (byte_idx, ch) in source.char_indices() {
        if line == target.line && character >= target.character {
            return Some(byte_idx);
        }
        if ch == '\n' {
            if line == target.line {
                return Some(byte_idx);
            }
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }

    if line == target.line && character >= target.character {
        Some(source.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_to_range_handles_multiline_offsets() {
        let source = "let x = 1\nlet y = x";
        assert_eq!(
            span_to_range(source, Span::new(13, 14)),
            Range::new(Position::new(1, 3), Position::new(1, 4))
        );
    }

    #[test]
    fn test_lsp_hover() {
        let mut type_map = HashMap::new();
        type_map.insert(Span::new(8, 10), "i32".to_string());
        let document = DocumentState {
            source: "let x = 42".to_string(),
            type_map,
        };

        let hover = hover_for_position(&document, Position::new(0, 9)).expect("hover result");
        assert_eq!(
            hover.contents,
            HoverContents::Scalar(MarkedString::String("type: i32".to_string()))
        );
    }
}
