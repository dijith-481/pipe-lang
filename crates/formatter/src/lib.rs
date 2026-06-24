use ast::ast::{BinOp, Decl, Expr, Pattern, Program, RecordPatternField, Stmt, TypeExpr, UnaryOp};

/// Formats a parsed program into a pretty-printed string.
/// Does NOT preserve comments (they are discarded during lexing).
#[must_use]
pub fn format(program: &Program) -> String {
    let mut out = String::new();
    for decl in program.decls.iter() {
        format_decl(decl, &mut out, 0);
        out.push('\n');
    }
    out
}

fn format_decl(decl: &Decl, out: &mut String, indent: usize) {
    match decl {
        Decl::Bind {
            name, ty, value, ..
        } => {
            out.push_str(&indent_str(indent));
            out.push_str("let ");
            out.push_str(name);
            if let Some(ann) = ty {
                out.push_str(": ");
                format_type_expr(ann, out);
            }
            out.push_str(" = ");
            format_expr(value, out, indent);
        }
        Decl::TypeAlias {
            name, params, rhs, ..
        } => {
            out.push_str(&indent_str(indent));
            out.push_str("type ");
            out.push_str(name);
            if !params.is_empty() {
                out.push('<');
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(p);
                }
                out.push('>');
            }
            out.push_str(" = ");
            format_type_expr(rhs, out);
        }
        Decl::Use { path, .. } => {
            out.push_str(&indent_str(indent));
            out.push_str("use ");
            out.push_str(&path.join("::"));
        }
    }
}

fn format_expr(expr: &Expr, out: &mut String, indent: usize) {
    match expr {
        Expr::IntLiteral(s, _) => out.push_str(s),
        Expr::FloatLiteral(s, _) => out.push_str(s),
        Expr::Str(s, _) => {
            out.push('"');
            out.push_str(s);
            out.push('"');
        }
        Expr::Bool(true, _) => out.push_str("true"),
        Expr::Bool(false, _) => out.push_str("false"),
        Expr::Ident(s, _) => out.push_str(s),
        Expr::Lambda {
            params,
            return_type,
            body,
            ..
        } => {
            out.push('(');
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(p.name);
                if let Some(ann) = &p.ty {
                    out.push_str(": ");
                    format_type_expr(ann, out);
                }
            }
            out.push(')');
            if let Some(ret) = return_type {
                out.push_str(": ");
                format_type_expr(ret, out);
            }
            out.push_str(" => ");
            format_expr(body, out, indent);
        }
        Expr::Application { func, args, .. } => {
            format_expr(func, out, indent);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_expr(a, out, indent);
            }
            out.push(')');
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            format_expr(left, out, indent);
            out.push(' ');
            out.push_str(match op {
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
            });
            out.push(' ');
            format_expr(right, out, indent);
        }
        Expr::Block { stmts, result, .. } => {
            out.push_str("{\n");
            for s in stmts.iter() {
                format_stmt(s, out, indent + 1);
                out.push('\n');
            }
            out.push_str(&indent_str(indent + 1));
            format_expr(result, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            out.push_str("if ");
            format_expr(condition, out, indent);
            out.push_str(" {\n");
            out.push_str(&indent_str(indent + 1));
            format_expr(then_branch, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push_str("} else {\n");
            out.push_str(&indent_str(indent + 1));
            format_expr(else_branch, out, indent + 1);
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::Match { subject, arms, .. } => {
            out.push_str("match ");
            format_expr(subject, out, indent);
            out.push_str(" {\n");
            for arm in arms.iter() {
                out.push_str(&indent_str(indent + 1));
                format_pattern(arm.pattern, out);
                out.push_str(" => ");
                format_expr(arm.body, out, indent + 1);
                out.push_str(",\n");
            }
            out.push_str(&indent_str(indent));
            out.push('}');
        }
        Expr::Array { elems, .. } => {
            out.push('[');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_expr(e, out, indent);
            }
            out.push(']');
        }
        Expr::Tuple { elems, .. } => {
            out.push('(');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_expr(e, out, indent);
            }
            out.push(')');
        }
        Expr::Record { fields, .. } => {
            out.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(f.name);
                out.push_str(": ");
                format_expr(f.value, out, indent);
            }
            out.push_str(" }");
        }
        Expr::FieldAccess { object, field, .. } => {
            format_expr(object, out, indent);
            out.push('.');
            out.push_str(field);
        }
        Expr::Index { array, index, .. } => {
            format_expr(array, out, indent);
            out.push('[');
            format_expr(index, out, indent);
            out.push(']');
        }
        Expr::Template { parts, .. } => {
            out.push('`');
            for part in parts.iter() {
                match part {
                    ast::ast::TemplatePart::Str(s) => out.push_str(s),
                    ast::ast::TemplatePart::Expr(e) => {
                        out.push_str("${");
                        format_expr(e, out, indent);
                        out.push('}');
                    }
                }
            }
            out.push('`');
        }
        Expr::Unary { op, operand, .. } => {
            out.push_str(match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            });
            format_expr(operand, out, indent);
        }
    }
}

fn format_stmt(stmt: &Stmt, out: &mut String, indent: usize) {
    match stmt {
        Stmt::Let { pattern, value } => {
            out.push_str(&indent_str(indent));
            out.push_str("let ");
            format_pattern(pattern, out);
            out.push_str(" = ");
            format_expr(value, out, indent);
        }
        Stmt::Expr(expr) => {
            out.push_str(&indent_str(indent));
            format_expr(expr, out, indent);
        }
    }
}

fn format_pattern(pattern: &Pattern, out: &mut String) {
    match pattern {
        Pattern::Wildcard(_) => out.push('_'),
        Pattern::Binding(name, _) => out.push_str(name),
        Pattern::Literal(lit, _) => match lit {
            ast::ast::LiteralPattern::Int(s) => out.push_str(s),
            ast::ast::LiteralPattern::Float(s) => out.push_str(s),
            ast::ast::LiteralPattern::Str(s) => {
                out.push('"');
                out.push_str(s);
                out.push('"');
            }
            ast::ast::LiteralPattern::Bool(true) => out.push_str("true"),
            ast::ast::LiteralPattern::Bool(false) => out.push_str("false"),
        },
        Pattern::Constructor { name, fields, .. } => {
            out.push_str(name);
            if !fields.is_empty() {
                out.push('(');
                for (i, a) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    format_pattern(a, out);
                }
                out.push(')');
            }
        }
        Pattern::Tuple { patterns, .. } => {
            out.push('(');
            for (i, e) in patterns.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_pattern(e, out);
            }
            out.push(')');
        }
        Pattern::Record { fields, .. } => {
            out.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_record_pattern_field(f, out);
            }
            out.push_str(" }");
        }
    }
}

fn format_record_pattern_field(field: &RecordPatternField, out: &mut String) {
    out.push_str(field.name);
    if let Some(p) = &field.pattern {
        out.push_str(": ");
        format_pattern(p, out);
    }
}

fn format_type_expr(ty: &TypeExpr, out: &mut String) {
    match ty {
        TypeExpr::Named(name, _) => out.push_str(name),
        TypeExpr::Apply { func, arg, .. } => {
            format_type_expr(func, out);
            out.push('<');
            format_type_expr(arg, out);
            out.push('>');
        }
        TypeExpr::Function { from, to, .. } => {
            format_type_expr(from, out);
            out.push_str(" -> ");
            format_type_expr(to, out);
        }
        TypeExpr::Tuple { types, .. } => {
            out.push('(');
            for (i, t) in types.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                format_type_expr(t, out);
            }
            out.push(')');
        }
        TypeExpr::Record { fields, .. } => {
            out.push_str("{ ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(f.name);
                out.push_str(": ");
                format_type_expr(f.ty, out);
            }
            out.push_str(" }");
        }
        TypeExpr::Sum { variants, .. } => {
            for (i, v) in variants.iter().enumerate() {
                if i > 0 {
                    out.push_str(" | ");
                }
                out.push_str(v.name);
                if !v.fields.is_empty() {
                    out.push('(');
                    for (j, t) in v.fields.iter().enumerate() {
                        if j > 0 {
                            out.push_str(", ");
                        }
                        format_type_expr(t, out);
                    }
                    out.push(')');
                }
            }
        }
    }
}

fn indent_str(level: usize) -> String {
    "    ".repeat(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::parse;

    #[test]
    fn formats_simple_let_binding() {
        let src = "let   x   =   42";
        let bump = bumpalo::Bump::new();
        let program = parse(src, &bump).unwrap();
        let out = format(&program);
        assert_eq!(out, "let x = 42\n");
    }

    #[test]
    fn formats_block_with_indentation() {
        let src = "let f = (x) => {\nlet y = x + 1\ny * 2\n}";
        let bump = bumpalo::Bump::new();
        let program = parse(src, &bump).unwrap();
        let out = format(&program);
        assert!(out.contains("    let y = x + 1"));
        assert!(out.contains("    y * 2"));
    }

    #[test]
    fn formats_if_else_with_braces() {
        let src = "let abs = (x) => if x > 0 { x } else { -x }";
        let bump = bumpalo::Bump::new();
        let program = parse(src, &bump).unwrap();
        let out = format(&program);
        assert!(out.contains("if ") && out.contains("} else {"));
    }

    #[test]
    fn formats_templates() {
        let src = "let  msg  = `Hello, ${  name  }!`";
        let bump = bumpalo::Bump::new();
        let program = parse(src, &bump).unwrap();
        let out = format(&program);
        assert!(out.contains("`Hello, ${name}!`"));
    }

    #[test]
    fn formats_match_expression() {
        let src = "let  r  = match   x   {  _   => 0 }";
        let bump = bumpalo::Bump::new();
        let program = parse(src, &bump).unwrap();
        let out = format(&program);
        assert!(out.contains("match"));
        assert!(out.contains("=>"));
    }
}
