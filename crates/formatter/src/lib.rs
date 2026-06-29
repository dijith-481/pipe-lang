use ast::ast::{BinOp, Decl, Expr, LiteralPattern, Pattern, Program, Stmt, TemplatePart, TypeExpr,
               UnaryOp};

/// Formats a pipe-lang source file with consistent indentation and spacing.
///
/// Comments are preserved by extracting them from the source before parsing
/// and re-inserting them at their approximate positions in the output.
pub fn format_source(source: &str) -> Result<String, String> {
    let arena = bumpalo::Bump::new();
    let program = parser::parse(source, &arena).map_err(|e| e.to_string())?;

    // Extract comments (line-based: // ...)
    let comments = extract_comments(source);

    let mut fmt = Fmt::new();
    fmt.format_program(&program, &comments);
    Ok(fmt.out)
}

/// A comment extracted from the source: its line number (0-indexed) and text.
#[derive(Debug, Clone)]
struct Comment {
    /// 0-indexed line number in the original source
    line: usize,
    /// The comment text including `//` prefix and leading whitespace
    text: String,
}

/// Extracts all single-line comments from source, preserving their positions.
fn extract_comments(source: &str) -> Vec<Comment> {
    source
        .lines()
        .enumerate()
        .filter_map(|(line_idx, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                Some(Comment {
                    line: line_idx,
                    text: line.to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

struct Fmt {
    out: String,
    indent: usize,
    /// Approximate output line number (for comment re-insertion tracking)
    out_line: usize,
}

impl Fmt {
    fn new() -> Self {
        Self {
            out: String::with_capacity(1024),
            indent: 0,
            out_line: 0,
        }
    }

    fn nl(&mut self) {
        self.out.push('\n');
        self.out_line += 1;
    }

    fn indent_str(&self) -> String {
        "  ".repeat(self.indent)
    }

    fn push_indent(&mut self) {
        self.indent += 1;
    }

    fn pop_indent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn ws(&mut self) {
        self.out.push(' ');
    }

    fn fmt(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Emit any comments that belong at the current output line.
    /// Advances `comments` past the inserted ones.
    fn emit_comments(&mut self, comments: &mut &[Comment]) {
        while let Some((first, rest)) = comments.split_first() {
            if first.line <= self.out_line.saturating_add(1) {
                let indent = self.indent_str();
                self.out.push_str(&indent);
                self.out.push_str(&first.text);
                self.nl();
                *comments = rest;
            } else {
                break;
            }
        }
    }

    fn format_program(&mut self, program: &Program, comments: &[Comment]) {
        let mut remaining = comments;
        for decl in &program.decls {
            self.emit_comments(&mut remaining);
            self.format_decl(decl);
            self.nl();
        }
        // Emit trailing comments
        self.emit_comments(&mut remaining);
    }

    fn format_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Bind { name, ty, value, .. } => {
                self.fmt("let ");
                self.fmt(name);
                if let Some(ty) = ty {
                    self.fmt(" : ");
                    self.format_type_expr(ty);
                }
                self.fmt(" = ");
                self.format_expr(value);
            }
            Decl::TypeAlias { name, params, rhs, .. } => {
                self.fmt("type ");
                self.fmt(name);
                if !params.is_empty() {
                    self.fmt("<");
                    for (i, p) in params.iter().enumerate() {
                        if i > 0 {
                            self.fmt(", ");
                        }
                        self.fmt(p);
                    }
                    self.fmt(">");
                }
                self.fmt(" = ");
                self.format_type_expr(rhs);
            }
            Decl::Use { path, .. } => {
                self.fmt("use ");
                for (i, seg) in path.iter().enumerate() {
                    if i > 0 {
                        self.fmt("::");
                    }
                    self.fmt(seg);
                }
            }
        }
    }

    fn format_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(name, _) => self.fmt(name),
            TypeExpr::Apply { func, arg, .. } => {
                self.format_type_expr(func);
                self.fmt("<");
                self.format_type_expr(arg);
                self.fmt(">");
            }
            TypeExpr::Function { from, to, .. } => {
                let is_func = matches!(&**from, TypeExpr::Function { .. });
                if is_func {
                    self.fmt("(");
                    self.format_type_expr(from);
                    self.fmt(")");
                } else {
                    self.format_type_expr(from);
                }
                self.fmt(" -> ");
                self.format_type_expr(to);
            }
            TypeExpr::Tuple { types, .. } => {
                self.fmt("(");
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.format_type_expr(t);
                }
                self.fmt(")");
            }
            TypeExpr::Record { fields, .. } => {
                if fields.is_empty() {
                    self.fmt("{}");
                } else {
                    self.fmt("{\n");
                    self.out_line += 1;
                    self.push_indent();
                    for field in fields {
                        self.out.push_str(&self.indent_str());
                        self.fmt(field.name);
                        self.fmt(" : ");
                        self.format_type_expr(field.ty);
                        self.fmt("\n");
                        self.out_line += 1;
                    }
                    self.pop_indent();
                    self.out.push_str(&self.indent_str());
                    self.fmt("}");
                }
            }
            TypeExpr::Sum { variants, .. } => {
                for v in variants.iter() {
                    self.fmt("| ");
                    self.fmt(v.name);
                    if !v.fields.is_empty() {
                        self.fmt("(");
                        for (j, f) in v.fields.iter().enumerate() {
                            if j > 0 {
                                self.fmt(", ");
                            }
                            self.format_type_expr(f);
                        }
                        self.fmt(")");
                    }
                    self.ws();
                }
            }
        }
    }

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLiteral(_, text, _) => self.fmt(text),
            Expr::FloatLiteral(_, text, _) => self.fmt(text),
            Expr::Str(_, val, _) => {
                self.fmt("\"");
                self.fmt(val);
                self.fmt("\"");
            }
            Expr::Bool(_, val, _) => {
                if *val { self.fmt("true"); } else { self.fmt("false"); }
            }
            Expr::Ident(_, name, _) => self.fmt(name),
            Expr::Application { func, args, .. } => {
                self.format_expr(func);
                self.fmt("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.format_expr(arg);
                }
                self.fmt(")");
            }
            Expr::Lambda { params, return_type, body, .. } => {
                self.fmt("(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.fmt(p.name);
                    if let Some(ty) = p.ty {
                        self.fmt(": ");
                        self.format_type_expr(ty);
                    }
                }
                self.fmt(")");
                if let Some(rt) = return_type {
                    self.fmt(": ");
                    self.format_type_expr(rt);
                }
                self.fmt(" => ");
                self.format_expr(body);
            }
            Expr::Binary { op, left, right, .. } => {
                self.format_expr(left);
                self.ws();
                self.fmt(op_str(op));
                self.ws();
                self.format_expr(right);
            }
            Expr::Unary { op, operand, .. } => {
                self.fmt(unary_op_str(op));
                self.format_expr(operand);
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                self.fmt("if ");
                self.format_expr(condition);
                self.fmt(" {");
                self.nl();
                self.push_indent();
                self.out.push_str(&self.indent_str());
                self.format_expr(then_branch);
                self.nl();
                self.pop_indent();
                self.out.push_str(&self.indent_str());
                self.fmt("} else {");
                self.nl();
                self.push_indent();
                self.out.push_str(&self.indent_str());
                self.format_expr(else_branch);
                self.nl();
                self.pop_indent();
                self.out.push_str(&self.indent_str());
                self.fmt("}");
            }
            Expr::Match { subject, arms, .. } => {
                self.fmt("match ");
                self.format_expr(subject);
                self.fmt(" {");
                self.nl();
                self.push_indent();
                for arm in arms {
                    self.out.push_str(&self.indent_str());
                    self.format_pattern(arm.pattern);
                    self.fmt(" => ");
                    self.format_expr(arm.body);
                    self.nl();
                }
                self.pop_indent();
                self.out.push_str(&self.indent_str());
                self.fmt("}");
            }
            Expr::Block { stmts, result, .. } => {
                self.fmt("{");
                self.nl();
                self.push_indent();
                for stmt in stmts {
                    self.out.push_str(&self.indent_str());
                    match stmt {
                        Stmt::Let { pattern, value } => {
                            self.fmt("let ");
                            self.format_pattern(pattern);
                            self.fmt(" = ");
                            self.format_expr(value);
                            self.fmt(";");
                            self.nl();
                        }
                        Stmt::Expr(e) => {
                            self.format_expr(e);
                            self.fmt(";");
                            self.nl();
                        }
                    }
                }
                self.out.push_str(&self.indent_str());
                self.format_expr(result);
                self.nl();
                self.pop_indent();
                self.out.push_str(&self.indent_str());
                self.fmt("}");
            }
            Expr::Record { fields, .. } => {
                if fields.is_empty() {
                    self.fmt("{}");
                } else {
                    self.fmt("{ ");
                    for (i, f) in fields.iter().enumerate() {
                        if i > 0 {
                            self.fmt(", ");
                        }
                        self.fmt(f.name);
                        self.fmt(": ");
                        self.format_expr(f.value);
                    }
                    self.fmt(" }");
                }
            }
            Expr::FieldAccess { object, field, .. } => {
                self.format_expr(object);
                self.fmt(".");
                self.fmt(field);
            }
            Expr::Tuple { elems, .. } => {
                self.fmt("(");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.format_expr(e);
                }
                self.fmt(")");
            }
            Expr::Array { elems, .. } => {
                if elems.is_empty() {
                    self.fmt("[]");
                } else {
                    self.fmt("[");
                    for (i, e) in elems.iter().enumerate() {
                        if i > 0 {
                            self.fmt(", ");
                        }
                        self.format_expr(e);
                    }
                    self.fmt("]");
                }
            }
            Expr::Template { parts, .. } => {
                self.fmt("`");
                for part in parts {
                    match part {
                        TemplatePart::Str(s) => self.fmt(s),
                        TemplatePart::Expr(e) => {
                            self.fmt("${");
                            self.format_expr(e);
                            self.fmt("}");
                        }
                    }
                }
                self.fmt("`");
            }
            Expr::Index { array, index, .. } => {
                self.format_expr(array);
                self.fmt("[");
                self.format_expr(index);
                self.fmt("]");
            }
        }
    }

    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard(_, _) => self.fmt("_"),
            Pattern::Literal(_, lit, _) => match lit {
                LiteralPattern::Int(s) => self.fmt(s),
                LiteralPattern::Float(s) => self.fmt(s),
                LiteralPattern::Str(s) => {
                    self.fmt("\"");
                    self.fmt(s);
                    self.fmt("\"");
                }
                LiteralPattern::Bool(b) => {
                    if *b { self.fmt("true"); } else { self.fmt("false"); }
                }
            },
            Pattern::Binding(_, name, _) => self.fmt(name),
            Pattern::Constructor { name, fields, .. } => {
                self.fmt(name);
                self.fmt("(");
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.format_pattern(f);
                }
                self.fmt(")");
            }
            Pattern::Tuple { patterns, .. } => {
                self.fmt("(");
                for (i, p) in patterns.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.format_pattern(p);
                }
                self.fmt(")");
            }
            Pattern::Record { fields, .. } => {
                self.fmt("{ ");
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        self.fmt(", ");
                    }
                    self.fmt(f.name);
                    if let Some(p) = f.pattern {
                        self.fmt(": ");
                        self.format_pattern(p);
                    }
                }
                self.fmt(" }");
            }
        }
    }
}

fn op_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn unary_op_str(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}
