pub mod env;
pub mod error;
pub mod infer;
pub mod types;
pub mod unify;

pub use crate::env::TypeEnv;
pub use crate::error::TypeError;
pub use crate::infer::{infer_decl, infer_expr};
pub use crate::types::{MonoType, PolyType, TypeId};
pub use crate::unify::{Substitution, unify};
