#![allow(deprecated)]
use std::collections::HashMap;
use std::sync::Arc;

use ast::ast::{Decl, Expr, MatchArm, Param, Pattern, Stmt, TemplatePart, TypeExpr};
use ast::span::Span;
use bumpalo::Bump;
use diagnostics::CompilerError;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

// ---------------------------------------------------------------------------
// Semantic token configuration
// ---------------------------------------------------------------------------

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::TYPE,
    SemanticTokenType::NUMBER,
    SemanticTokenType::STRING,
];

const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::DEFINITION,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::STATIC,
];

#[derive(Clone, Copy)]
enum TokenType {
    Keyword = 0,
    Function = 1,
    Variable = 2,
    Parameter = 3,
    Type = 4,
    Number = 5,
    String = 6,
}

#[derive(Clone, Copy)]
#[expect(dead_code)]
enum TokenModifier {
    Declaration = 1,
    Definition = 2,
    Readonly = 4,
    Static = 8,
}

const KEYWORDS: &[&str] = &["let", "type", "use", "match", "if", "else", "true", "false"];

// ---------------------------------------------------------------------------
// Document state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DocumentState {
    source: Arc<str>,
    type_map: HashMap<Span, String>,
}

// ---------------------------------------------------------------------------
// LSP Backend
// ---------------------------------------------------------------------------

pub struct Backend {
    client: Client,
    documents: tokio::sync::RwLock<HashMap<Url, DocumentState>>,
}

impl Backend {
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    async fn analyze_and_publish(&self, uri: Url, source: String) {
        let (type_map, diagnostics) = analyze_source(&source);
        self.documents.write().await.insert(
            uri.clone(),
            DocumentState {
                source: source.into(),
                type_map,
            },
        );
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

// ---------------------------------------------------------------------------
// LanguageServer trait implementation
// ---------------------------------------------------------------------------

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: TOKEN_MODIFIERS.to_vec(),
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..SemanticTokensOptions::default()
                        },
                    ),
                ),
                document_formatting_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    InlayHintOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                ))),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    work_done_progress_options: Default::default(),
                }),
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
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let offset = match position_to_byte_offset(&document.source, position) {
            Some(o) => o,
            None => return Ok(None),
        };
        let ty = document
            .type_map
            .iter()
            .filter(|(span, _)| span.contains(offset))
            .min_by_key(|(span, _)| span.len())
            .map(|(_, ty)| ty.clone());
        match ty {
            Some(t) => Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!("type: {t}"))),
                range: None,
            })),
            None => Ok(None),
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let trigger = params
            .context
            .and_then(|c| c.trigger_character)
            .unwrap_or_default();

        if trigger == "." {
            // Could offer field completions — not yet implemented
            return Ok(None);
        }

        let items: Vec<CompletionItem> = KEYWORDS
            .iter()
            .enumerate()
            .map(|(i, kw)| CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                sort_text: Some(format!("{:03}", i)),
                ..CompletionItem::default()
            })
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let offset = match position_to_byte_offset(&document.source, position) {
            Some(o) => o,
            None => return Ok(None),
        };
        let source = document.source.clone();
        drop(documents);
        let arena = Bump::new();
        let program = match parser::parse(&source, &arena) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        match find_definition(&program.decls, &source, offset) {
            Some(def_span) => {
                let range = span_to_range(&source, def_span);
                Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri,
                    range,
                })))
            }
            None => Ok(None),
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(d) => d.clone(),
            None => return Ok(None),
        };
        drop(documents);
        let arena = Bump::new();
        let program = match parser::parse(&document.source, &arena) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let symbols: Vec<DocumentSymbol> = program
            .decls
            .iter()
            .filter_map(|decl| decl_symbol(decl, &document.source))
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(d) => d.clone(),
            None => return Ok(None),
        };
        drop(documents);
        let source = &document.source;
        let arena = Bump::new();
        let program = match parser::parse(source, &arena) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let mut builder = SemanticTokenBuilder::new(source);
        for decl in program.decls.iter() {
            walk_decl_for_tokens(decl, source, &mut builder);
        }
        Ok(Some(SemanticTokensResult::Tokens(builder.build())))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let source = match documents.get(&uri) {
            Some(d) => d.source.clone(),
            None => return Ok(None),
        };
        drop(documents);
        let arena = Bump::new();
        let program = match parser::parse(&source, &arena) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let formatted = formatter::format(&program);
        let line_count = source.lines().count();
        let last_line_len = source.lines().last().map_or(0, |l| l.len());
        let full_range = Range::new(
            Position::new(0, 0),
            Position::new(line_count as u32 - 1, last_line_len as u32),
        );
        Ok(Some(vec![TextEdit {
            range: full_range,
            new_text: formatted,
        }]))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let documents = self.documents.read().await;
        let document = match documents.get(&uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let source = &document.source;
        let hints: Vec<InlayHint> = document
            .type_map
            .iter()
            .filter(|(span, _)| !span.is_empty())
            .filter_map(|(span, ty)| {
                let span_range = span_to_range(source, *span);
                if !ranges_overlap(&range, &span_range) {
                    return None;
                }
                // Skip whole-line spans like "let x = 42"
                let text = span.source_text(source);
                let trimmed = text.trim();
                if trimmed.starts_with("let ")
                    || trimmed.starts_with("type ")
                    || trimmed.starts_with("use ")
                    || trimmed.starts_with("if ")
                    || trimmed.starts_with("match ")
                {
                    return None;
                }
                let pos = byte_offset_to_position(source, span.end);
                Some(InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!(": {ty}")),
                    kind: Some(InlayHintKind::TYPE),
                    padding_left: Some(true),
                    padding_right: None,
                    tooltip: None,
                    text_edits: None,
                    data: None,
                })
            })
            .collect();
        Ok(Some(hints))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let documents = self.documents.read().await;
        let source = match documents.get(&uri) {
            Some(d) => d.source.clone(),
            None => return Ok(None),
        };
        drop(documents);
        let arena = Bump::new();
        let program = match parser::parse(&source, &arena) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let offset = match position_to_byte_offset(&source, position) {
            Some(o) => o,
            None => return Ok(None),
        };
        let call_info = match find_call_at(program.decls.iter(), &source, offset) {
            Some(c) => c,
            None => return Ok(None),
        };
        let params: Vec<ParameterInformation> = call_info
            .param_names
            .iter()
            .map(|name| ParameterInformation {
                label: ParameterLabel::Simple(name.clone()),
                documentation: None,
            })
            .collect();
        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: call_info.label,
                documentation: None,
                parameters: Some(params),
                active_parameter: Some(call_info.active_param),
            }],
            active_signature: Some(0),
            active_parameter: Some(call_info.active_param),
        }))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

fn analyze_source(source: &str) -> (HashMap<Span, String>, Vec<Diagnostic>) {
    let arena = Bump::new();
    let program = match parser::parse(source, &arena) {
        Ok(p) => p,
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
                .map(|e| compiler_error_to_lsp(source, CompilerError::from(e)))
                .collect(),
        ),
    }
}

// ---------------------------------------------------------------------------
// Go-to-definition helpers
// ---------------------------------------------------------------------------

fn find_definition(decls: &[Decl<'_>], source: &str, offset: usize) -> Option<Span> {
    let name = identifier_at(decls, source, offset)?;
    for decl in decls {
        match decl {
            Decl::Bind { name: n, .. } if *n == name => return Some(decl_span(decl)),
            Decl::TypeAlias { name: n, .. } if *n == name => return Some(decl_span(decl)),
            _ => {}
        }
    }
    None
}

fn identifier_at<'a>(decls: &'a [Decl<'a>], source: &'a str, offset: usize) -> Option<&'a str> {
    for decl in decls {
        match decl {
            Decl::Bind {
                name, value, span, ..
            } => {
                if offset >= span.start && offset < span.start + 4 + name.len() {
                    // Check if offset is within the name portion
                    let name_start = span.start + 4; // "let " is 4 chars
                    let name_end = name_start + name.len();
                    if offset >= name_start && offset < name_end {
                        return Some(name);
                    }
                }
                if let Some(id) = expr_identifier(value, source, offset) {
                    return Some(id);
                }
            }
            Decl::TypeAlias { name, span, .. } => {
                let name_start = span.start + 5; // "type " is 5 chars
                let name_end = name_start + name.len();
                if offset >= name_start && offset < name_end {
                    return Some(name);
                }
            }
            Decl::Use { path, span, .. } => {
                for part in path.iter() {
                    if let Some(s) = find_ident_span(source, part, span.start)
                        && s.contains(offset)
                    {
                        return Some(part);
                    }
                }
            }
        }
    }
    None
}

fn expr_identifier<'a>(expr: &Expr<'a>, _source: &str, offset: usize) -> Option<&'a str> {
    match expr {
        Expr::Ident(name, span) if span.contains(offset) => Some(name),
        Expr::Application { func, args, .. } => {
            if let Some(id) = expr_identifier(func, _source, offset) {
                return Some(id);
            }
            for arg in args.iter() {
                if let Some(id) = expr_identifier(arg, _source, offset) {
                    return Some(id);
                }
            }
            None
        }
        Expr::Binary { left, right, .. } => expr_identifier(left, _source, offset)
            .or_else(|| expr_identifier(right, _source, offset)),
        Expr::Unary { operand, .. } => expr_identifier(operand, _source, offset),
        Expr::Lambda { body, .. } => expr_identifier(body, _source, offset),
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => expr_identifier(condition, _source, offset)
            .or_else(|| expr_identifier(then_branch, _source, offset))
            .or_else(|| expr_identifier(else_branch, _source, offset)),
        Expr::Block { stmts, result, .. } => {
            for stmt in stmts.iter() {
                if let Stmt::Let { value, .. } = stmt {
                    if let Some(id) = expr_identifier(value, _source, offset) {
                        return Some(id);
                    }
                } else if let Stmt::Expr(e) = stmt
                    && let Some(id) = expr_identifier(e, _source, offset)
                {
                    return Some(id);
                }
            }
            expr_identifier(result, _source, offset)
        }
        Expr::Match { subject, arms, .. } => {
            if let Some(id) = expr_identifier(subject, _source, offset) {
                return Some(id);
            }
            for arm in arms.iter() {
                if let Some(id) = expr_identifier(arm.body, _source, offset) {
                    return Some(id);
                }
            }
            None
        }
        Expr::Array { elems, .. } | Expr::Tuple { elems, .. } => {
            for e in elems.iter() {
                if let Some(id) = expr_identifier(e, _source, offset) {
                    return Some(id);
                }
            }
            None
        }
        Expr::Record { fields, .. } => {
            for f in fields.iter() {
                if let Some(id) = expr_identifier(f.value, _source, offset) {
                    return Some(id);
                }
            }
            None
        }
        Expr::FieldAccess { object, .. } => expr_identifier(object, _source, offset),
        Expr::Index { array, index, .. } => expr_identifier(array, _source, offset)
            .or_else(|| expr_identifier(index, _source, offset)),
        Expr::Template { parts, .. } => {
            for part in parts.iter() {
                if let TemplatePart::Expr(e) = part
                    && let Some(id) = expr_identifier(e, _source, offset)
                {
                    return Some(id);
                }
            }
            None
        }
        _ => None,
    }
}

fn find_ident_span(source: &str, name: &str, offset: usize) -> Option<Span> {
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(name_bytes) {
            let start = i;
            let end = i + name_bytes.len();
            if offset >= start && offset < end {
                return Some(Span::new(start, end));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Document symbol helpers
// ---------------------------------------------------------------------------

fn decl_symbol(decl: &Decl<'_>, source: &str) -> Option<DocumentSymbol> {
    match decl {
        Decl::Bind { name, value, .. } => {
            let range = span_to_range(source, decl_span(decl));
            let selection_range = span_to_range(source, decl_span(decl));
            let children = expr_children_symbols(value, source);
            Some(DocumentSymbol {
                name: name.to_string(),
                kind: SymbolKind::VARIABLE,
                range,
                selection_range,
                children: if children.is_empty() {
                    None
                } else {
                    Some(children)
                },
                detail: None,
                tags: None,
                deprecated: None,
            })
        }
        Decl::TypeAlias { name, .. } => {
            let range = span_to_range(source, decl_span(decl));
            let selection_range = span_to_range(source, decl_span(decl));
            Some(DocumentSymbol {
                name: name.to_string(),
                kind: SymbolKind::NAMESPACE,
                range,
                selection_range,
                children: None,
                detail: None,
                tags: None,
                deprecated: None,
            })
        }
        Decl::Use { path, .. } => {
            let range = span_to_range(source, decl_span(decl));
            let selection_range = span_to_range(source, decl_span(decl));
            Some(DocumentSymbol {
                name: path.join("::"),
                kind: SymbolKind::MODULE,
                range,
                selection_range,
                children: None,
                detail: None,
                tags: None,
                deprecated: None,
            })
        }
    }
}

fn expr_children_symbols(expr: &Expr<'_>, source: &str) -> Vec<DocumentSymbol> {
    match expr {
        Expr::Lambda { params, .. } => params
            .iter()
            .map(|p| {
                let range = span_to_range(source, param_span(p, source));
                DocumentSymbol {
                    name: p.name.to_string(),
                    kind: SymbolKind::VARIABLE,
                    range,
                    selection_range: range,
                    children: None,
                    detail: None,
                    tags: None,
                    deprecated: None,
                }
            })
            .collect(),
        Expr::Match { arms, .. } => arms
            .iter()
            .map(|arm| {
                let range = span_to_range(source, arm_span(arm));
                let name = pattern_name(arm.pattern).unwrap_or("_").to_string();
                DocumentSymbol {
                    name,
                    kind: SymbolKind::CONSTRUCTOR,
                    range,
                    selection_range: range,
                    children: None,
                    detail: None,
                    tags: None,
                    deprecated: None,
                }
            })
            .collect(),
        _ => vec![],
    }
}

fn pattern_name<'a>(pat: &'a Pattern<'a>) -> Option<&'a str> {
    match pat {
        Pattern::Binding(name, _) => Some(name),
        Pattern::Constructor { name, .. } => Some(name),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Semantic token helpers
// ---------------------------------------------------------------------------

struct SemanticTokenBuilder<'a> {
    source: &'a str,
    tokens: Vec<SemanticToken>,
    prev_line: u32,
    prev_start: u32,
}

impl<'a> SemanticTokenBuilder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            tokens: Vec::new(),
            prev_line: 0,
            prev_start: 0,
        }
    }

    fn add(&mut self, span: Span, token_type: TokenType, modifiers: u32) {
        let start = byte_offset_to_position(self.source, span.start);
        let delta_line = start.line - self.prev_line;
        let delta_start = if delta_line == 0 {
            start.character - self.prev_start
        } else {
            start.character
        };
        let end = byte_offset_to_position(self.source, span.end);
        let length = end.character - start.character;
        self.tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type: token_type as u32,
            token_modifiers_bitset: modifiers,
        });
        self.prev_line = start.line;
        self.prev_start = start.character;
    }

    fn build(self) -> SemanticTokens {
        SemanticTokens {
            result_id: None,
            data: self.tokens,
        }
    }
}

fn walk_decl_for_tokens(decl: &Decl<'_>, source: &str, builder: &mut SemanticTokenBuilder<'_>) {
    match decl {
        Decl::Bind {
            name,
            ty,
            value,
            span,
        } => {
            // The `let` keyword
            if let Some(kw_span) = keyword_span(source, *span, "let") {
                builder.add(kw_span, TokenType::Keyword, 0);
            }
            // The binding name
            if let Some(name_span) = find_ident_span(source, name, span.start + 4) {
                builder.add(
                    name_span,
                    TokenType::Function,
                    TokenModifier::Definition as u32,
                );
            }
            // Type annotation
            if let Some(ann) = ty {
                walk_type_for_tokens(ann, source, builder);
            }
            // The `=` sign
            if let Some(eq_span) = symbol_span(source, *span, "=") {
                builder.add(eq_span, TokenType::Keyword, 0);
            }
            // The value expression
            walk_expr_for_tokens(value, source, builder);
        }
        Decl::TypeAlias {
            name,
            params,
            rhs,
            span,
        } => {
            if let Some(kw_span) = keyword_span(source, *span, "type") {
                builder.add(kw_span, TokenType::Keyword, 0);
            }
            if let Some(name_span) = find_ident_span(source, name, span.start + 5) {
                builder.add(name_span, TokenType::Type, TokenModifier::Definition as u32);
            }
            for p in params.iter() {
                if let Some(ps) = find_ident_span(source, p, span.start) {
                    builder.add(ps, TokenType::Type, 0);
                }
            }
            walk_type_for_tokens(rhs, source, builder);
        }
        Decl::Use { path, span } => {
            if let Some(kw_span) = keyword_span(source, *span, "use") {
                builder.add(kw_span, TokenType::Keyword, 0);
            }
            for part in path.iter() {
                if let Some(ps) = find_ident_span(source, part, span.start) {
                    builder.add(ps, TokenType::Type, 0);
                }
            }
        }
    }
}

fn walk_expr_for_tokens(expr: &Expr<'_>, source: &str, builder: &mut SemanticTokenBuilder<'_>) {
    match expr {
        Expr::IntLiteral(_, span) => {
            builder.add(*span, TokenType::Number, 0);
        }
        Expr::FloatLiteral(_, span) => {
            builder.add(*span, TokenType::Number, 0);
        }
        Expr::Str(_, span) => {
            builder.add(*span, TokenType::String, 0);
        }
        Expr::Bool(_, span) => {
            builder.add(*span, TokenType::Keyword, 0);
        }
        Expr::Ident(name, span) => {
            // Check if it's a keyword
            if KEYWORDS.contains(name) {
                builder.add(*span, TokenType::Keyword, 0);
            } else {
                // Determine if it's a function or variable by checking if it starts a call
                builder.add(*span, TokenType::Variable, 0);
            }
        }
        Expr::Lambda {
            params,
            return_type,
            body,
            ..
        } => {
            for p in params.iter() {
                if let Some(ps) = find_ident_span(source, p.name, 0) {
                    builder.add(ps, TokenType::Parameter, TokenModifier::Definition as u32);
                }
                if let Some(ty) = &p.ty {
                    walk_type_for_tokens(ty, source, builder);
                }
            }
            if let Some(ret) = return_type {
                // The `:` and return type
                if let Some(colon) = symbol_span(source, ret.span(), ":") {
                    builder.add(colon, TokenType::Keyword, 0);
                }
                walk_type_for_tokens(ret, source, builder);
            }
            walk_expr_for_tokens(body, source, builder);
        }
        Expr::Application { func, args, .. } => {
            // Mark the function name
            if let Expr::Ident(_, span) = func {
                builder.add(*span, TokenType::Function, 0);
            } else {
                walk_expr_for_tokens(func, source, builder);
            }
            for arg in args.iter() {
                walk_expr_for_tokens(arg, source, builder);
            }
        }
        Expr::Binary {
            op: _, left, right, ..
        } => {
            walk_expr_for_tokens(left, source, builder);
            walk_expr_for_tokens(right, source, builder);
        }
        Expr::Unary { operand, .. } => {
            walk_expr_for_tokens(operand, source, builder);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => {
            if let Some(kw) = keyword_span(source, *span, "if") {
                builder.add(kw, TokenType::Keyword, 0);
            }
            walk_expr_for_tokens(condition, source, builder);
            if let Some(kw) = keyword_span(source, *span, "else") {
                builder.add(kw, TokenType::Keyword, 0);
            }
            walk_expr_for_tokens(then_branch, source, builder);
            walk_expr_for_tokens(else_branch, source, builder);
        }
        Expr::Block { stmts, result, .. } => {
            for stmt in stmts.iter() {
                match stmt {
                    Stmt::Let { pattern, value } => {
                        if let Some(ps) = pattern_span(pattern) {
                            builder.add(ps, TokenType::Variable, TokenModifier::Definition as u32);
                        }
                        walk_expr_for_tokens(value, source, builder);
                    }
                    Stmt::Expr(e) => walk_expr_for_tokens(e, source, builder),
                }
            }
            walk_expr_for_tokens(result, source, builder);
        }
        Expr::Match { subject, arms, .. } => {
            walk_expr_for_tokens(subject, source, builder);
            for arm in arms.iter() {
                walk_pattern_for_tokens(arm.pattern, source, builder);
                walk_expr_for_tokens(arm.body, source, builder);
            }
        }
        Expr::Array { elems, .. } | Expr::Tuple { elems, .. } => {
            for e in elems.iter() {
                walk_expr_for_tokens(e, source, builder);
            }
        }
        Expr::Record { fields, .. } => {
            for f in fields.iter() {
                if let Some(fs) = find_ident_span(source, f.name, 0) {
                    builder.add(fs, TokenType::Variable, 0);
                }
                walk_expr_for_tokens(f.value, source, builder);
            }
        }
        Expr::FieldAccess { object, field, .. } => {
            walk_expr_for_tokens(object, source, builder);
            if let Some(fs) = find_ident_span(source, field, 0) {
                builder.add(fs, TokenType::Variable, 0);
            }
        }
        Expr::Index { array, index, .. } => {
            walk_expr_for_tokens(array, source, builder);
            walk_expr_for_tokens(index, source, builder);
        }
        Expr::Template { parts, .. } => {
            for part in parts.iter() {
                if let TemplatePart::Expr(e) = part {
                    walk_expr_for_tokens(e, source, builder);
                }
            }
            // The entire template span
            builder.add(expr.span(), TokenType::String, 0);
        }
    }
}

fn walk_pattern_for_tokens(
    pat: &Pattern<'_>,
    source: &str,
    builder: &mut SemanticTokenBuilder<'_>,
) {
    match pat {
        Pattern::Wildcard(_) => {}
        Pattern::Binding(name, span) => {
            if let Some(ps) = find_ident_span(source, name, span.start) {
                builder.add(ps, TokenType::Variable, TokenModifier::Definition as u32);
            }
        }
        Pattern::Literal(_, _) => {}
        Pattern::Constructor { fields, .. } => {
            // The constructor name
            builder.add(pat.span(), TokenType::Function, 0);
            for f in fields.iter() {
                walk_pattern_for_tokens(f, source, builder);
            }
        }
        Pattern::Tuple { patterns, .. } => {
            for p in patterns.iter() {
                walk_pattern_for_tokens(p, source, builder);
            }
        }
        Pattern::Record { fields, .. } => {
            for f in fields.iter() {
                match &f.pattern {
                    Some(p) => walk_pattern_for_tokens(p, source, builder),
                    None => {
                        if let Some(fs) = find_ident_span(source, f.name, 0) {
                            builder.add(fs, TokenType::Variable, TokenModifier::Definition as u32);
                        }
                    }
                }
            }
        }
    }
}

fn walk_type_for_tokens(ty: &TypeExpr<'_>, source: &str, builder: &mut SemanticTokenBuilder<'_>) {
    match ty {
        TypeExpr::Named(name, span) => {
            builder.add(*span, TokenType::Type, 0);
            let _ = name;
        }
        TypeExpr::Apply { func, arg, .. } => {
            walk_type_for_tokens(func, source, builder);
            walk_type_for_tokens(arg, source, builder);
        }
        TypeExpr::Function { from, to, .. } => {
            walk_type_for_tokens(from, source, builder);
            walk_type_for_tokens(to, source, builder);
        }
        TypeExpr::Tuple { types, .. } => {
            for t in types.iter() {
                walk_type_for_tokens(t, source, builder);
            }
        }
        TypeExpr::Record { fields, .. } => {
            for f in fields.iter() {
                if let Some(fs) = find_ident_span(source, f.name, 0) {
                    builder.add(fs, TokenType::Variable, 0);
                }
                walk_type_for_tokens(f.ty, source, builder);
            }
        }
        TypeExpr::Sum { variants, .. } => {
            for v in variants.iter() {
                for t in v.fields.iter() {
                    walk_type_for_tokens(t, source, builder);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Signature help helpers
// ---------------------------------------------------------------------------

struct CallInfo {
    label: String,
    param_names: Vec<String>,
    active_param: u32,
}

fn find_call_at<'a>(
    decls: impl Iterator<Item = &'a Decl<'a>>,
    source: &str,
    offset: usize,
) -> Option<CallInfo> {
    for decl in decls {
        if let Decl::Bind { value, .. } = decl
            && let Some(info) = find_call_in_expr(value, source, offset)
        {
            return Some(info);
        }
    }
    None
}

fn find_call_in_expr(expr: &Expr<'_>, source: &str, offset: usize) -> Option<CallInfo> {
    match expr {
        Expr::Application { func, args, span } => {
            if span.contains(offset) {
                let func_name = match func {
                    Expr::Ident(name, _) => name.to_string(),
                    _ => "(anonymous)".to_string(),
                };
                let param_names: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("arg{i}"))
                    .collect();
                // Count commas before offset to find active parameter
                let source_slice = &source[span.start..offset.min(source.len())];
                let active_param = source_slice.matches(',').count() as u32;
                return Some(CallInfo {
                    label: format!("{}({})", func_name, param_names.join(", ")),
                    param_names,
                    active_param: active_param.min(args.len().saturating_sub(1) as u32),
                });
            }
            // Check in func or args recursively
            if let Some(info) = find_call_in_expr(func, source, offset) {
                return Some(info);
            }
            for arg in args.iter() {
                if let Some(info) = find_call_in_expr(arg, source, offset) {
                    return Some(info);
                }
            }
            None
        }
        Expr::Binary { left, right, .. } => find_call_in_expr(left, source, offset)
            .or_else(|| find_call_in_expr(right, source, offset)),
        Expr::Unary { operand, .. } => find_call_in_expr(operand, source, offset),
        Expr::Lambda { body, .. } => find_call_in_expr(body, source, offset),
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => find_call_in_expr(condition, source, offset)
            .or_else(|| find_call_in_expr(then_branch, source, offset))
            .or_else(|| find_call_in_expr(else_branch, source, offset)),
        Expr::Block { stmts, result, .. } => {
            for stmt in stmts.iter() {
                if let Stmt::Expr(e) = stmt
                    && let Some(info) = find_call_in_expr(e, source, offset)
                {
                    return Some(info);
                } else if let Stmt::Let { value, .. } = stmt
                    && let Some(info) = find_call_in_expr(value, source, offset)
                {
                    return Some(info);
                }
            }
            find_call_in_expr(result, source, offset)
        }
        Expr::Match { subject, arms, .. } => {
            if let Some(info) = find_call_in_expr(subject, source, offset) {
                return Some(info);
            }
            for arm in arms.iter() {
                if let Some(info) = find_call_in_expr(arm.body, source, offset) {
                    return Some(info);
                }
            }
            None
        }
        Expr::Array { elems, .. } | Expr::Tuple { elems, .. } => {
            for e in elems.iter() {
                if let Some(info) = find_call_in_expr(e, source, offset) {
                    return Some(info);
                }
            }
            None
        }
        Expr::Record { fields, .. } => {
            for f in fields.iter() {
                if let Some(info) = find_call_in_expr(f.value, source, offset) {
                    return Some(info);
                }
            }
            None
        }
        Expr::FieldAccess { object, .. } => find_call_in_expr(object, source, offset),
        Expr::Index { array, index, .. } => find_call_in_expr(array, source, offset)
            .or_else(|| find_call_in_expr(index, source, offset)),
        Expr::Template { parts, .. } => {
            for part in parts.iter() {
                if let TemplatePart::Expr(e) = part
                    && let Some(info) = find_call_in_expr(e, source, offset)
                {
                    return Some(info);
                }
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Span/position utilities
// ---------------------------------------------------------------------------

fn decl_span(decl: &Decl<'_>) -> Span {
    match decl {
        Decl::Bind { span, .. } | Decl::TypeAlias { span, .. } | Decl::Use { span, .. } => *span,
    }
}

fn param_span(param: &Param<'_>, source: &str) -> Span {
    find_ident_span(source, param.name, 0).unwrap_or(Span::new(0, 0))
}

fn arm_span(arm: &MatchArm<'_>) -> Span {
    arm.pattern.span()
}

fn pattern_span(pat: &Pattern<'_>) -> Option<Span> {
    match pat {
        Pattern::Binding(_, span) => Some(*span),
        _ => None,
    }
}

fn keyword_span(source: &str, parent: Span, keyword: &str) -> Option<Span> {
    let start = parent.start;
    let end = parent.end;
    let slice = &source[start..end.min(source.len())];
    slice
        .find(keyword)
        .map(|pos| Span::new(start + pos, start + pos + keyword.len()))
}

fn symbol_span(source: &str, parent: Span, sym: &str) -> Option<Span> {
    let start = parent.start;
    let end = parent.end;
    let slice = &source[start..end.min(source.len())];
    slice
        .find(sym)
        .map(|pos| Span::new(start + pos, start + pos + sym.len()))
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

fn ranges_overlap(a: &Range, b: &Range) -> bool {
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character <= b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character <= a.start.character))
}

// ---------------------------------------------------------------------------
// LSP diagnostic conversion
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn test_hover() {
        let mut type_map = HashMap::new();
        type_map.insert(Span::new(8, 10), "i32".to_string());
        let document = DocumentState {
            source: "let x = 42".into(),
            type_map,
        };
        let offset =
            position_to_byte_offset(&document.source, Position::new(0, 9)).expect("byte offset");
        let ty = document
            .type_map
            .iter()
            .filter(|(span, _)| span.contains(offset))
            .min_by_key(|(span, _)| span.len())
            .map(|(_, ty)| ty.clone());
        assert_eq!(ty.as_deref(), Some("i32"));
    }

    #[test]
    fn test_completion_keywords() {
        // Test that all keywords are in the list
        assert!(KEYWORDS.contains(&"let"));
        assert!(KEYWORDS.contains(&"match"));
        assert!(KEYWORDS.contains(&"if"));
    }

    #[test]
    fn test_ranges_overlap() {
        let a = Range::new(Position::new(0, 0), Position::new(0, 10));
        let b = Range::new(Position::new(0, 5), Position::new(0, 15));
        assert!(ranges_overlap(&a, &b));
        let c = Range::new(Position::new(1, 0), Position::new(1, 10));
        assert!(!ranges_overlap(&a, &c));
    }

    #[test]
    fn inlay_hint_shows_type_after_expression() {
        let mut type_map = HashMap::new();
        type_map.insert(Span::new(8, 10), "i32".to_string());
        let document = DocumentState {
            source: "let x = 42".into(),
            type_map,
        };
        let source = &document.source;
        let range = Range::new(Position::new(0, 0), Position::new(0, 10));
        let hints: Vec<InlayHint> = document
            .type_map
            .iter()
            .filter(|(span, _)| !span.is_empty())
            .filter_map(|(span, ty)| {
                let span_range = span_to_range(source, *span);
                if !ranges_overlap(&range, &span_range) {
                    return None;
                }
                let text = span.source_text(source);
                let trimmed = text.trim();
                if trimmed.starts_with("let ")
                    || trimmed.starts_with("type ")
                    || trimmed.starts_with("use ")
                {
                    return None;
                }
                let pos = byte_offset_to_position(source, span.end);
                Some(InlayHint {
                    position: pos,
                    label: InlayHintLabel::String(format!(": {ty}")),
                    kind: Some(InlayHintKind::TYPE),
                    padding_left: Some(true),
                    padding_right: None,
                    tooltip: None,
                    text_edits: None,
                    data: None,
                })
            })
            .collect();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].position, Position::new(0, 10));
        if let InlayHintLabel::String(ref s) = hints[0].label {
            assert_eq!(s, ": i32");
        } else {
            panic!("expected string label");
        }
    }
}
