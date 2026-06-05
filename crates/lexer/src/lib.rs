pub mod error;
pub mod lexer;

pub use crate::error::LexError;
pub use crate::lexer::{Lexer, Token, TokenKind};
pub use ast::SmolStr;
