use std::collections::HashMap;

use ast::SmolStr;

use crate::types::{MonoType, PolyType, TypeId};

/// Maps a tag type name to its variant definitions: (variant_name, payload_types).
pub type TagVariants = std::collections::HashMap<SmolStr, Vec<(SmolStr, Vec<MonoType>)>>;

/// A scoped type environment for tracking type bindings.
///
/// Supports nested scopes via `push_scope` / `pop_scope`.
/// Lookups search from innermost to outermost scope.
#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<HashMap<SmolStr, PolyType>>,
    next_type_id: u32,
    /// Full variant structure for each tag type (e.g. Option → [None, Some(T)]).
    pub tag_variants: TagVariants,
}

impl TypeEnv {
    /// Creates a new type environment with a single global scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_type_id: 0,
            tag_variants: HashMap::new(),
        }
    }

    /// Allocates a fresh type variable.
    pub fn fresh_var(&mut self) -> TypeId {
        let id = TypeId(self.next_type_id);
        self.next_type_id += 1;
        id
    }

    /// Pushes a new empty scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pops the innermost scope.
    ///
    /// # Panics
    ///
    /// Panics if there is only one scope left (the global scope).
    pub fn pop_scope(&mut self) {
        assert!(self.scopes.len() > 1, "cannot pop the global scope");
        self.scopes.pop();
    }

    /// Inserts a type binding in the current (innermost) scope.
    pub fn insert(&mut self, name: impl Into<SmolStr>, ty: PolyType) {
        self.scopes
            .last_mut()
            .expect("scope stack is never empty")
            .insert(name.into(), ty);
    }

    /// Looks up a type by name, searching from innermost to outermost scope.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&PolyType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Returns true if the given name is bound in any scope.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Returns the number of active scopes.
    #[must_use]
    pub fn scope_depth(&self) -> usize {
        self.scopes.len()
    }

    /// Iterates over all polytypes currently visible in the environment.
    ///
    /// Used by `generalize` to compute the set of free variables in scope.
    pub fn all_types(&self) -> impl Iterator<Item = &PolyType> {
        self.scopes.iter().flat_map(|scope| scope.values())
    }

    /// Loads the prelude into the global scope.
    ///
    /// The prelude contains core types (`Option`, `Result`) and utility
    /// functions (`id`, `const`, `flip`, `compose`, `pipe`, `apply`)
    /// that are automatically available in every pipe-lang program.
    pub fn load_prelude(&mut self) {
        use crate::types::MonoType;
        use std::rc::Rc;

        // Load Option<T> as a sum type
        let opt_a = self.fresh_var();
        let option_type = PolyType::poly(
            vec![opt_a],
            MonoType::Tag {
                name: "Option".into(),
                payload: Rc::from([MonoType::Var(opt_a)]),
            },
        );
        self.insert("Option", option_type);

        // Load Result<T, E> as a sum type
        let res_t = self.fresh_var();
        let res_e = self.fresh_var();
        let result_type = PolyType::poly(
            vec![res_t, res_e],
            MonoType::Tag {
                name: "Result".into(),
                payload: Rc::from([MonoType::Var(res_t), MonoType::Var(res_e)]),
            },
        );
        self.insert("Result", result_type);

        // --- Sum type constructors ---

        // Some : <a>(a) -> Option<a>
        let some_a = self.fresh_var();
        let some_type = PolyType::poly(
            vec![some_a],
            MonoType::Func {
                params: Rc::from([MonoType::Var(some_a)]),
                ret: Rc::new(MonoType::Tag {
                    name: "Option".into(),
                    payload: Rc::from([MonoType::Var(some_a)]),
                }),
            },
        );
        self.insert("Some", some_type);

        // None : <a>Option<a>  (bare value, not a function)
        let none_a = self.fresh_var();
        let none_type = PolyType::poly(
            vec![none_a],
            MonoType::Tag {
                name: "Option".into(),
                payload: Rc::from([MonoType::Var(none_a)]),
            },
        );
        self.insert("None", none_type);

        // Ok : <a, b>(a) -> Result<a, b>
        let ok_a = self.fresh_var();
        let ok_b = self.fresh_var();
        let ok_type = PolyType::poly(
            vec![ok_a, ok_b],
            MonoType::Func {
                params: Rc::from([MonoType::Var(ok_a)]),
                ret: Rc::new(MonoType::Tag {
                    name: "Result".into(),
                    payload: Rc::from([MonoType::Var(ok_a), MonoType::Var(ok_b)]),
                }),
            },
        );
        self.insert("Ok", ok_type);

        // Err : <a, b>(b) -> Result<a, b>
        let err_a = self.fresh_var();
        let err_b = self.fresh_var();
        let err_type = PolyType::poly(
            vec![err_a, err_b],
            MonoType::Func {
                params: Rc::from([MonoType::Var(err_b)]),
                ret: Rc::new(MonoType::Tag {
                    name: "Result".into(),
                    payload: Rc::from([MonoType::Var(err_a), MonoType::Var(err_b)]),
                }),
            },
        );
        self.insert("Err", err_type);

        // Populate tag variant info for sum types.
        self.tag_variants.insert(
            "Option".into(),
            vec![
                ("None".into(), vec![]),
                ("Some".into(), vec![MonoType::Var(opt_a)]),
            ],
        );
        self.tag_variants.insert(
            "Result".into(),
            vec![
                ("Ok".into(), vec![MonoType::Var(res_t)]),
                ("Err".into(), vec![MonoType::Var(res_e)]),
            ],
        );

        // Load core utility function types
        // id : <a>(a) -> a
        let a = self.fresh_var();
        let id_type = PolyType::poly(
            vec![a],
            MonoType::Func {
                params: Rc::from([MonoType::Var(a)]),
                ret: Rc::new(MonoType::Var(a)),
            },
        );
        self.insert("id", id_type);

        // const : <a, b>(a) -> (b) -> a
        let ca = self.fresh_var();
        let cb = self.fresh_var();
        let const_type = PolyType::poly(
            vec![ca, cb],
            MonoType::Func {
                params: Rc::from([MonoType::Var(ca)]),
                ret: Rc::new(MonoType::Func {
                    params: Rc::from([MonoType::Var(cb)]),
                    ret: Rc::new(MonoType::Var(ca)),
                }),
            },
        );
        self.insert("const", const_type);

        // flip : <a, b, c>((a, b) -> c) -> (b, a) -> c
        let fa = self.fresh_var();
        let fb = self.fresh_var();
        let fc = self.fresh_var();
        let flip_type = PolyType::poly(
            vec![fa, fb, fc],
            MonoType::Func {
                params: Rc::from([MonoType::Func {
                    params: Rc::from([MonoType::Var(fa), MonoType::Var(fb)]),
                    ret: Rc::new(MonoType::Var(fc)),
                }]),
                ret: Rc::new(MonoType::Func {
                    params: Rc::from([MonoType::Var(fb), MonoType::Var(fa)]),
                    ret: Rc::new(MonoType::Var(fc)),
                }),
            },
        );
        self.insert("flip", flip_type);

        // compose : <a, b, c>((b) -> c, (a) -> b) -> (a) -> c
        let comp_a = self.fresh_var();
        let comp_b = self.fresh_var();
        let comp_c = self.fresh_var();
        let compose_type = PolyType::poly(
            vec![comp_a, comp_b, comp_c],
            MonoType::Func {
                params: Rc::from([
                    MonoType::Func {
                        params: Rc::from([MonoType::Var(comp_b)]),
                        ret: Rc::new(MonoType::Var(comp_c)),
                    },
                    MonoType::Func {
                        params: Rc::from([MonoType::Var(comp_a)]),
                        ret: Rc::new(MonoType::Var(comp_b)),
                    },
                ]),
                ret: Rc::new(MonoType::Func {
                    params: Rc::from([MonoType::Var(comp_a)]),
                    ret: Rc::new(MonoType::Var(comp_c)),
                }),
            },
        );
        self.insert("compose", compose_type);

        // pipe : <a, b>((a) -> b, (b) -> c) -> (a) -> c
        // Simplified: <a, b>((a) -> b) -> ... (chained, variadic in practice)
        // For now, just declare it as a generic function type
        let pipe_a = self.fresh_var();
        let pipe_b = self.fresh_var();
        let pipe_c = self.fresh_var();
        let pipe_type = PolyType::poly(
            vec![pipe_a, pipe_b, pipe_c],
            MonoType::Func {
                params: Rc::from([
                    MonoType::Func {
                        params: Rc::from([MonoType::Var(pipe_a)]),
                        ret: Rc::new(MonoType::Var(pipe_b)),
                    },
                    MonoType::Func {
                        params: Rc::from([MonoType::Var(pipe_b)]),
                        ret: Rc::new(MonoType::Var(pipe_c)),
                    },
                ]),
                ret: Rc::new(MonoType::Func {
                    params: Rc::from([MonoType::Var(pipe_a)]),
                    ret: Rc::new(MonoType::Var(pipe_c)),
                }),
            },
        );
        self.insert("pipe", pipe_type);

        // apply : <a, b>((a) -> b, a) -> b
        let app_a = self.fresh_var();
        let app_b = self.fresh_var();
        let apply_type = PolyType::poly(
            vec![app_a, app_b],
            MonoType::Func {
                params: Rc::from([
                    MonoType::Func {
                        params: Rc::from([MonoType::Var(app_a)]),
                        ret: Rc::new(MonoType::Var(app_b)),
                    },
                    MonoType::Var(app_a),
                ]),
                ret: Rc::new(MonoType::Var(app_b)),
            },
        );
        self.insert("apply", apply_type);

        // ====================================================================
        // I/O builtins
        // ====================================================================

        // println : (str) -> Effect<()>
        self.insert(
            "println",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Effect(Box::new(MonoType::Unit))),
            }),
        );

        // print : (str) -> Effect<()>
        self.insert(
            "print",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Effect(Box::new(MonoType::Unit))),
            }),
        );

        // read_line : () -> Effect<str>
        self.insert(
            "read_line",
            PolyType::mono(MonoType::Func {
                params: Rc::from([]),
                ret: Rc::new(MonoType::Effect(Box::new(MonoType::Str))),
            }),
        );

        // read_file : (str) -> Effect<Result<str, str>>
        // Result payload: [T, E] = [str, str]
        self.insert(
            "read_file",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Effect(Box::new(MonoType::Tag {
                    name: "Result".into(),
                    payload: Rc::from([MonoType::Str, MonoType::Str]),
                }))),
            }),
        );

        // ====================================================================
        // Effect combinators
        // ====================================================================

        // Effect.map : <a, b>(Effect<a>, (a) -> b) -> Effect<b>
        let em_a = self.fresh_var();
        let em_b = self.fresh_var();
        self.insert(
            "Effect.map",
            PolyType::poly(
                vec![em_a, em_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Effect(Box::new(MonoType::Var(em_a))),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(em_a)]),
                            ret: Rc::new(MonoType::Var(em_b)),
                        },
                    ]),
                    ret: Rc::new(MonoType::Effect(Box::new(MonoType::Var(em_b)))),
                },
            ),
        );

        // Effect.flat_map : <a, b>(Effect<a>, (a) -> Effect<b>) -> Effect<b>
        let efm_a = self.fresh_var();
        let efm_b = self.fresh_var();
        self.insert(
            "Effect.flat_map",
            PolyType::poly(
                vec![efm_a, efm_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Effect(Box::new(MonoType::Var(efm_a))),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(efm_a)]),
                            ret: Rc::new(MonoType::Effect(Box::new(MonoType::Var(efm_b)))),
                        },
                    ]),
                    ret: Rc::new(MonoType::Effect(Box::new(MonoType::Var(efm_b)))),
                },
            ),
        );

        // ====================================================================
        // Array builtins
        // ====================================================================

        // map : <a, b>(Array<a>, (a) -> b) -> Array<b>
        let map_a = self.fresh_var();
        let map_b = self.fresh_var();
        self.insert(
            "map",
            PolyType::poly(
                vec![map_a, map_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(map_a))),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(map_a)]),
                            ret: Rc::new(MonoType::Var(map_b)),
                        },
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(map_b)))),
                },
            ),
        );

        // filter : <a>(Array<a>, (a) -> Bool) -> Array<a>
        let filter_a = self.fresh_var();
        self.insert(
            "filter",
            PolyType::poly(
                vec![filter_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(filter_a))),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(filter_a)]),
                            ret: Rc::new(MonoType::Bool),
                        },
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(filter_a)))),
                },
            ),
        );

        // fold : <a, b>(Array<a>, b, (b, a) -> b) -> b
        let fold_a = self.fresh_var();
        let fold_b = self.fresh_var();
        self.insert(
            "fold",
            PolyType::poly(
                vec![fold_a, fold_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(fold_a))),
                        MonoType::Var(fold_b),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(fold_b), MonoType::Var(fold_a)]),
                            ret: Rc::new(MonoType::Var(fold_b)),
                        },
                    ]),
                    ret: Rc::new(MonoType::Var(fold_b)),
                },
            ),
        );

        // flat_map : <a, b>(Array<a>, (a) -> Array<b>) -> Array<b>
        let flata = self.fresh_var();
        let flatb = self.fresh_var();
        self.insert(
            "flat_map",
            PolyType::poly(
                vec![flata, flatb],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(flata))),
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(flata)]),
                            ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(flatb)))),
                        },
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(flatb)))),
                },
            ),
        );

        // len : <a>(Array<a>) -> Usize
        let len_a = self.fresh_var();
        self.insert(
            "len",
            PolyType::poly(
                vec![len_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(len_a)))]),
                    ret: Rc::new(MonoType::Usize),
                },
            ),
        );

        // concat : <a>(Array<a>, Array<a>) -> Array<a>
        let concat_a = self.fresh_var();
        self.insert(
            "concat",
            PolyType::poly(
                vec![concat_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(concat_a))),
                        MonoType::Array(Rc::new(MonoType::Var(concat_a))),
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(concat_a)))),
                },
            ),
        );

        // prepend : <a>(Array<a>, a) -> Array<a>
        let prepend_a = self.fresh_var();
        self.insert(
            "prepend",
            PolyType::poly(
                vec![prepend_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(prepend_a))),
                        MonoType::Var(prepend_a),
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(prepend_a)))),
                },
            ),
        );

        // head : <a>(Array<a>) -> Option<a>
        let head_a = self.fresh_var();
        self.insert(
            "head",
            PolyType::poly(
                vec![head_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(head_a)))]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Option".into(),
                        payload: Rc::from([MonoType::Var(head_a)]),
                    }),
                },
            ),
        );

        // tail : <a>(Array<a>) -> Option<Array<a>>
        let tail_a = self.fresh_var();
        self.insert(
            "tail",
            PolyType::poly(
                vec![tail_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(tail_a)))]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Option".into(),
                        payload: Rc::from([MonoType::Array(Rc::new(MonoType::Var(tail_a)))]),
                    }),
                },
            ),
        );

        // ====================================================================
        // String methods
        // ====================================================================

        // Str.concat : (str, str) -> str
        self.insert(
            "Str.concat",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str, MonoType::Str]),
                ret: Rc::new(MonoType::Str),
            }),
        );

        // Str.len : (str) -> Usize
        self.insert(
            "Str.len",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Usize),
            }),
        );

        // Str.split : (str, str) -> Array<str>
        self.insert(
            "Str.split",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str, MonoType::Str]),
                ret: Rc::new(MonoType::Array(Rc::new(MonoType::Str))),
            }),
        );
        // Bare alias: split(str, str) -> Array<str>
        self.insert(
            "split",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str, MonoType::Str]),
                ret: Rc::new(MonoType::Array(Rc::new(MonoType::Str))),
            }),
        );

        // Str.trim : (str) -> str
        self.insert(
            "Str.trim",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Str),
            }),
        );
        // Bare alias: trim(str) -> str
        self.insert(
            "trim",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Str),
            }),
        );

        // Str.parse_i32 : (str) -> Result<i32, str>
        self.insert(
            "Str.parse_i32",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Tag {
                    name: "Result".into(),
                    payload: Rc::from([MonoType::I32, MonoType::Str]),
                }),
            }),
        );
        // Bare alias: parse_i32(str) -> Result<i32, str>
        self.insert(
            "parse_i32",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::Str]),
                ret: Rc::new(MonoType::Tag {
                    name: "Result".into(),
                    payload: Rc::from([MonoType::I32, MonoType::Str]),
                }),
            }),
        );

        // ====================================================================
        // Option methods
        // ====================================================================

        // Option.map : <a, b>(Option<a>, (a) -> b) -> Option<b>
        let om_a = self.fresh_var();
        let om_b = self.fresh_var();
        self.insert(
            "Option.map",
            PolyType::poly(
                vec![om_a, om_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(om_a)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(om_a)]),
                            ret: Rc::new(MonoType::Var(om_b)),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Option".into(),
                        payload: Rc::from([MonoType::Var(om_b)]),
                    }),
                },
            ),
        );

        // Option.flat_map : <a, b>(Option<a>, (a) -> Option<b>) -> Option<b>
        let ofm_a = self.fresh_var();
        let ofm_b = self.fresh_var();
        self.insert(
            "Option.flat_map",
            PolyType::poly(
                vec![ofm_a, ofm_b],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(ofm_a)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(ofm_a)]),
                            ret: Rc::new(MonoType::Tag {
                                name: "Option".into(),
                                payload: Rc::from([MonoType::Var(ofm_b)]),
                            }),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Option".into(),
                        payload: Rc::from([MonoType::Var(ofm_b)]),
                    }),
                },
            ),
        );

        // Option.flatMap : <a, b>(Option<a>, (a) -> Option<b>) -> Option<b>
        let ofm_a2 = self.fresh_var();
        let ofm_b2 = self.fresh_var();
        self.insert(
            "Option.flatMap",
            PolyType::poly(
                vec![ofm_a2, ofm_b2],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(ofm_a2)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(ofm_a2)]),
                            ret: Rc::new(MonoType::Tag {
                                name: "Option".into(),
                                payload: Rc::from([MonoType::Var(ofm_b2)]),
                            }),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Option".into(),
                        payload: Rc::from([MonoType::Var(ofm_b2)]),
                    }),
                },
            ),
        );

        // Option.unwrapOr : <a>(Option<a>, a) -> a
        let uo_a = self.fresh_var();
        self.insert(
            "Option.unwrapOr",
            PolyType::poly(
                vec![uo_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(uo_a)]),
                        },
                        MonoType::Var(uo_a),
                    ]),
                    ret: Rc::new(MonoType::Var(uo_a)),
                },
            ),
        );
        // Bare alias: unwrap_or(Option<a>, a) -> a
        let uo2_a = self.fresh_var();
        self.insert(
            "unwrap_or",
            PolyType::poly(
                vec![uo2_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(uo2_a)]),
                        },
                        MonoType::Var(uo2_a),
                    ]),
                    ret: Rc::new(MonoType::Var(uo2_a)),
                },
            ),
        );
        // Option.unwrap_or : <a>(Option<a>, a) -> a (snake_case alias)
        let uo3_a = self.fresh_var();
        self.insert(
            "Option.unwrap_or",
            PolyType::poly(
                vec![uo3_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(uo3_a)]),
                        },
                        MonoType::Var(uo3_a),
                    ]),
                    ret: Rc::new(MonoType::Var(uo3_a)),
                },
            ),
        );

        // ====================================================================
        // Result methods
        // ====================================================================

        // Result.map : <t, e, u>(Result<t, e>, (t) -> u) -> Result<u, e>
        let rm_t = self.fresh_var();
        let rm_e = self.fresh_var();
        let rm_u = self.fresh_var();
        self.insert(
            "Result.map",
            PolyType::poly(
                vec![rm_t, rm_e, rm_u],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Result".into(),
                            payload: Rc::from([MonoType::Var(rm_t), MonoType::Var(rm_e)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(rm_t)]),
                            ret: Rc::new(MonoType::Var(rm_u)),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Result".into(),
                        payload: Rc::from([MonoType::Var(rm_u), MonoType::Var(rm_e)]),
                    }),
                },
            ),
        );

        // Result.flat_map : <t, e, u>(Result<t, e>, (t) -> Result<u, e>) -> Result<u, e>
        let rfm_t = self.fresh_var();
        let rfm_e = self.fresh_var();
        let rfm_u = self.fresh_var();
        self.insert(
            "Result.flat_map",
            PolyType::poly(
                vec![rfm_t, rfm_e, rfm_u],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Result".into(),
                            payload: Rc::from([MonoType::Var(rfm_t), MonoType::Var(rfm_e)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(rfm_t)]),
                            ret: Rc::new(MonoType::Tag {
                                name: "Result".into(),
                                payload: Rc::from([MonoType::Var(rfm_u), MonoType::Var(rfm_e)]),
                            }),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Result".into(),
                        payload: Rc::from([MonoType::Var(rfm_u), MonoType::Var(rfm_e)]),
                    }),
                },
            ),
        );

        // Result.flatMap : <t, e, u>(Result<t, e>, (t) -> Result<u, e>) -> Result<u, e>
        let rfm2_t = self.fresh_var();
        let rfm2_e = self.fresh_var();
        let rfm2_u = self.fresh_var();
        self.insert(
            "Result.flatMap",
            PolyType::poly(
                vec![rfm2_t, rfm2_e, rfm2_u],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Result".into(),
                            payload: Rc::from([MonoType::Var(rfm2_t), MonoType::Var(rfm2_e)]),
                        },
                        MonoType::Func {
                            params: Rc::from([MonoType::Var(rfm2_t)]),
                            ret: Rc::new(MonoType::Tag {
                                name: "Result".into(),
                                payload: Rc::from([MonoType::Var(rfm2_u), MonoType::Var(rfm2_e)]),
                            }),
                        },
                    ]),
                    ret: Rc::new(MonoType::Tag {
                        name: "Result".into(),
                        payload: Rc::from([MonoType::Var(rfm2_u), MonoType::Var(rfm2_e)]),
                    }),
                },
            ),
        );

        // Result.unwrapOr : <t, e>(Result<t, e>, t) -> t
        let ru_t = self.fresh_var();
        let ru_e = self.fresh_var();
        self.insert(
            "Result.unwrapOr",
            PolyType::poly(
                vec![ru_t, ru_e],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Result".into(),
                            payload: Rc::from([MonoType::Var(ru_t), MonoType::Var(ru_e)]),
                        },
                        MonoType::Var(ru_t),
                    ]),
                    ret: Rc::new(MonoType::Var(ru_t)),
                },
            ),
        );
        // Result.unwrap_or : <t, e>(Result<t, e>, t) -> t (snake_case alias)
        let ru2_t = self.fresh_var();
        let ru2_e = self.fresh_var();
        self.insert(
            "Result.unwrap_or",
            PolyType::poly(
                vec![ru2_t, ru2_e],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Result".into(),
                            payload: Rc::from([MonoType::Var(ru2_t), MonoType::Var(ru2_e)]),
                        },
                        MonoType::Var(ru2_t),
                    ]),
                    ret: Rc::new(MonoType::Var(ru2_t)),
                },
            ),
        );

        // ====================================================================
        // Numeric conversion functions
        // ====================================================================

        // to_i64 : <a>(a) -> i64  (polymorphic — works on any numeric type)
        let ti64_a = self.fresh_var();
        self.insert(
            "to_i64",
            PolyType::poly(
                vec![ti64_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Var(ti64_a)]),
                    ret: Rc::new(MonoType::I64),
                },
            ),
        );

        // to_i32 : <a>(a) -> i32  (polymorphic — truncates floats, widens integers)
        let ti32_a = self.fresh_var();
        self.insert(
            "to_i32",
            PolyType::poly(
                vec![ti32_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Var(ti32_a)]),
                    ret: Rc::new(MonoType::I32),
                },
            ),
        );

        // to_usize : (i32) -> Usize
        self.insert(
            "to_usize",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::I32]),
                ret: Rc::new(MonoType::Usize),
            }),
        );

        // to_f64 : <a>(a) -> f64  (polymorphic — works on any numeric type)
        let tf64_a = self.fresh_var();
        self.insert(
            "to_f64",
            PolyType::poly(
                vec![tf64_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Var(tf64_a)]),
                    ret: Rc::new(MonoType::F64),
                },
            ),
        );

        // to_str : <a>(a) -> str  (polymorphic — formats any primitive)
        let tstr_a = self.fresh_var();
        self.insert(
            "to_str",
            PolyType::poly(
                vec![tstr_a],
                MonoType::Func {
                    params: Rc::from([MonoType::Var(tstr_a)]),
                    ret: Rc::new(MonoType::Str),
                },
            ),
        );

        // I64.to_str : (i64) -> str
        self.insert(
            "I64.to_str",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::I64]),
                ret: Rc::new(MonoType::Str),
            }),
        );

        // F64.to_str : (f64) -> str
        self.insert(
            "F64.to_str",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::F64]),
                ret: Rc::new(MonoType::Str),
            }),
        );

        // ====================================================================
        // Additional builtins needed by example programs
        // ====================================================================

        // drop : <a>(Array<a>, Usize) -> Array<a>
        let drop_a = self.fresh_var();
        self.insert(
            "drop",
            PolyType::poly(
                vec![drop_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(drop_a))),
                        MonoType::Usize,
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(drop_a)))),
                },
            ),
        );

        // take : <a>(Array<a>, Usize) -> Array<a>
        let take_a = self.fresh_var();
        self.insert(
            "take",
            PolyType::poly(
                vec![take_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Array(Rc::new(MonoType::Var(take_a))),
                        MonoType::Usize,
                    ]),
                    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(take_a)))),
                },
            ),
        );

        // sqrt : (f64) -> f64
        self.insert(
            "sqrt",
            PolyType::mono(MonoType::Func {
                params: Rc::from([MonoType::F64]),
                ret: Rc::new(MonoType::F64),
            }),
        );

        // unwrap : <a>(Option<a>, a) -> a   (or Result, uses tag dispatch)
        let unwrap_a = self.fresh_var();
        self.insert(
            "unwrap",
            PolyType::poly(
                vec![unwrap_a],
                MonoType::Func {
                    params: Rc::from([
                        MonoType::Tag {
                            name: "Option".into(),
                            payload: Rc::from([MonoType::Var(unwrap_a)]),
                        },
                        MonoType::Var(unwrap_a),
                    ]),
                    ret: Rc::new(MonoType::Var(unwrap_a)),
                },
            ),
        );
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MonoType;

    #[test]
    fn lookup_in_global_scope() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn lookup_in_inner_scope() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        env.push_scope();
        env.insert("y", PolyType::mono(MonoType::Str));
        assert_eq!(env.lookup("y"), Some(&PolyType::mono(MonoType::Str)));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn pop_scope_removes_bindings() {
        let mut env = TypeEnv::new();
        env.push_scope();
        env.insert("x", PolyType::mono(MonoType::I32));
        assert!(env.contains("x"));
        env.pop_scope();
        assert!(!env.contains("x"));
    }

    #[test]
    fn inner_scope_shadows_outer() {
        let mut env = TypeEnv::new();
        env.insert("x", PolyType::mono(MonoType::I32));
        env.push_scope();
        env.insert("x", PolyType::mono(MonoType::F64));
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::F64)));
        env.pop_scope();
        assert_eq!(env.lookup("x"), Some(&PolyType::mono(MonoType::I32)));
    }

    #[test]
    fn fresh_var_increments() {
        let mut env = TypeEnv::new();
        let v1 = env.fresh_var();
        let v2 = env.fresh_var();
        assert_ne!(v1, v2);
    }

    #[test]
    fn scope_depth_tracks_correctly() {
        let mut env = TypeEnv::new();
        assert_eq!(env.scope_depth(), 1);
        env.push_scope();
        assert_eq!(env.scope_depth(), 2);
        env.pop_scope();
        assert_eq!(env.scope_depth(), 1);
    }

    #[test]
    fn load_prelude_adds_option() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("Option"));
    }

    #[test]
    fn load_prelude_adds_result() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("Result"));
    }

    #[test]
    fn load_prelude_adds_core_functions() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("id"));
        assert!(env.contains("const"));
        assert!(env.contains("flip"));
        assert!(env.contains("compose"));
        assert!(env.contains("pipe"));
        assert!(env.contains("apply"));
    }

    #[test]
    fn load_prelude_id_has_correct_type() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        let id_type = env.lookup("id").unwrap();
        assert!(id_type.quantified.len() == 1); // <a>
        assert!(matches!(id_type.body, MonoType::Func { .. }));
    }

    #[test]
    fn load_prelude_has_all_builtin_names() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        for name in &[
            "println",
            "print",
            "read_line",
            "read_file",
            "map",
            "filter",
            "fold",
            "flat_map",
            "concat",
            "prepend",
            "len",
            "head",
            "tail",
            "Str.concat",
            "Str.len",
            "Str.split",
            "Str.trim",
            "Str.parse_i32",
            "Option.map",
            "Option.flatMap",
            "Option.unwrapOr",
            "Option.unwrap_or",
            "Result.map",
            "Result.flatMap",
            "Result.unwrapOr",
            "Result.unwrap_or",
            "Effect.map",
            "Effect.flat_map",
            "to_i64",
            "to_i32",
            "to_f64",
            "to_str",
            "I64.to_str",
            "F64.to_str",
            "drop",
            "take",
            "sqrt",
            "unwrap",
        ] {
            assert!(env.contains(name), "missing prelude binding: {name}");
        }
    }

    #[test]
    fn load_prelude_map_is_polymorphic() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        let map_ty = env.lookup("map").unwrap();
        assert_eq!(map_ty.quantified.len(), 2);
    }

    #[test]
    fn load_prelude_option_map_is_qualified() {
        let mut env = TypeEnv::new();
        env.load_prelude();
        assert!(env.contains("Option.map"));
    }
}
