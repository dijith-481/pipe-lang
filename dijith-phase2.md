# Dijith — Phase 2: Frontend Fixes (Typechecker + Parser)

**Crates:** `crates/typechecker`, `crates/parser`, `crates/ir`, `crates/ast`  
**This must be done FIRST — blocks all other members**  
**Goal:** Fix all root causes so all 14 example programs typecheck AND lower to valid IR

---

## Dependency Chain

```
dijith-phase2 (you are here) → member1-phase2 (JIT can then run typechecked programs)
                             → member2-phase2 (stdlib/type signatures align with fixed prelude)
                             → member3-phase2 (tooling can build on working pipeline)
```

Without these fixes, all other work is blocked because 12/14 example programs fail at typecheck.

---

## Current State (from audit)

| Root Cause | File | Line | Impact |
|---|---|---|---|
| 1. Typechecker prelude has only 12 of 33+ builtins | `typechecker/src/env.rs` | 98 | `println`, `map`, `filter`, `fold`, `len`, `concat` etc. all fail as "unbound variable" |
| 2. `Decl::TypeAlias` ignores RHS, stores `Unit` | `typechecker/src/infer.rs` | 798 | `type Person = { name: str, age: i32 }` registers `Person : Unit`. Variant constructors never registered. |
| 3. `type_expr_to_mono` can't resolve user types | `typechecker/src/infer.rs` | 106–124 | `Person`, `a` (generic param) etc. all fail. Hardcoded 12-primitive list. |
| 4. Multi-param `(str, i32) -> str` becomes 1 tuple param | `typechecker/src/infer.rs` | 126–129 | Function with 2 params becomes `Func{params:[Tag("Tuple",[Str,I32])], ret:Str}` |

Each of these is a single file change. Fixing all 4 unblocks all 14 programs.

---

## Task 1: Register All Missing Prelude Type Signatures

**File:** `crates/typechecker/src/env.rs`, function `load_prelude()`

### Current (line 198–312): 6 utility functions (id, const, flip, compose, pipe, apply) + 2 sum types (Option, Result) + 4 constructors (Some, None, Ok, Err) = 12 bindings.

### What to add: Register these 27 additional bindings, following the same `PolyType::poly(quantified_vars, MonoType::Func { ... })` pattern.

**I/O builtins (3):**
```rust
// println : (str) -> ()
let println_ty = PolyType::poly(
    vec![],
    MonoType::Func {
        params: Rc::from([MonoType::Str]),
        ret: Rc::new(MonoType::Unit),
    },
);
self.insert("println", println_ty);

// print : (str) -> ()
let print_ty = /* same as println */;
self.insert("print", print_ty);

// read_line : () -> str
let read_line_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([]),
    ret: Rc::new(MonoType::Str),
});
self.insert("read_line", read_line_ty);
```

**Array builtins (8):**
```rust
// map    : <a, b>(Array<a>, (a) -> b) -> Array<b>
let map_a = self.fresh_var();
let map_b = self.fresh_var();
let map_ty = PolyType::poly(
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
);
self.insert("map", map_ty);

// filter : <a>(Array<a>, (a) -> Bool) -> Array<a>
let filter_a = self.fresh_var();
let filter_ty = PolyType::poly(
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
);
self.insert("filter", filter_ty);

// fold   : <a, b>(Array<a>, b, (b, a) -> b) -> b
let fold_a = self.fresh_var();
let fold_b = self.fresh_var();
let fold_ty = PolyType::poly(
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
);
self.insert("fold", fold_ty);

// flatMap : <a, b>(Array<a>, (a) -> Array<b>) -> Array<b>
let flat_a = self.fresh_var();
let flat_b = self.fresh_var();
let flat_map_ty = PolyType::poly(
    vec![flat_a, flat_b],
    MonoType::Func {
        params: Rc::from([
            MonoType::Array(Rc::new(MonoType::Var(flat_a))),
            MonoType::Func {
                params: Rc::from([MonoType::Var(flat_a)]),
                ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(flat_b)))),
            },
        ]),
        ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(flat_b)))),
    },
);
self.insert("flatMap", flat_map_ty);

// concat  : <a>(Array<a>, Array<a>) -> Array<a>
let concat_a = self.fresh_var();
let concat_ty = PolyType::poly(
    vec![concat_a],
    MonoType::Func {
        params: Rc::from([
            MonoType::Array(Rc::new(MonoType::Var(concat_a))),
            MonoType::Array(Rc::new(MonoType::Var(concat_a))),
        ]),
        ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(concat_a)))),
    },
);
self.insert("concat", concat_ty);

// prepend : <a>(Array<a>, a) -> Array<a>
let prepend_a = self.fresh_var();
let prepend_ty = PolyType::poly(
    vec![prepend_a],
    MonoType::Func {
        params: Rc::from([
            MonoType::Array(Rc::new(MonoType::Var(prepend_a))),
            MonoType::Var(prepend_a),
        ]),
        ret: Rc::new(MonoType::Array(Rc::new(MonoType::Var(prepend_a)))),
    },
);
self.insert("prepend", prepend_ty);

// len     : <a>(Array<a>) -> Usize
let len_a = self.fresh_var();
let len_ty = PolyType::poly(
    vec![len_a],
    MonoType::Func {
        params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(len_a)))]),
        ret: Rc::new(MonoType::Usize),
    },
);
self.insert("len", len_ty);

// head    : <a>(Array<a>) -> Option<a>
let head_a = self.fresh_var();
let head_ty = PolyType::poly(
    vec![head_a],
    MonoType::Func {
        params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(head_a)))]),
        ret: Rc::new(MonoType::Tag {
            name: "Option".into(),
            payload: Rc::from([MonoType::Var(head_a)]),
        }),
    },
);
self.insert("head", head_ty);

// tail    : <a>(Array<a>) -> Option<Array<a>>
let tail_a = self.fresh_var();
let tail_ty = PolyType::poly(
    vec![tail_a],
    MonoType::Func {
        params: Rc::from([MonoType::Array(Rc::new(MonoType::Var(tail_a)))]),
        ret: Rc::new(MonoType::Tag {
            name: "Option".into(),
            payload: Rc::from([MonoType::Array(Rc::new(MonoType::Var(tail_a)))]),
        }),
    },
);
self.insert("tail", tail_ty);
```

**String methods (5):**
```rust
// Str.concat  : (str, str) -> str
let str_concat_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str, MonoType::Str]),
    ret: Rc::new(MonoType::Str),
});
self.insert("Str.concat", str_concat_ty);

// Str.len     : (str) -> Usize
let str_len_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str]),
    ret: Rc::new(MonoType::Usize),
});
self.insert("Str.len", str_len_ty);

// Str.split   : (str, str) -> Array<str>
let str_split_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str, MonoType::Str]),
    ret: Rc::new(MonoType::Array(Rc::new(MonoType::Str))),
});
self.insert("Str.split", str_split_ty);

// Str.trim    : (str) -> str
let str_trim_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str]),
    ret: Rc::new(MonoType::Str),
});
self.insert("Str.trim", str_trim_ty);

// Str.parse_i32 : (str) -> Result<i32, str>
let str_parse_i32_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str]),
    ret: Rc::new(MonoType::Tag {
        name: "Result".into(),
        payload: Rc::from([MonoType::I32, MonoType::Str]),
    }),
});
self.insert("Str.parse_i32", str_parse_i32_ty);
```

**Option methods (3):**
```rust
// Option.map       : <a, b>(Option<a>, (a) -> b) -> Option<b>
let om_a = self.fresh_var();
let om_b = self.fresh_var();
let opt_map_ty = PolyType::poly(
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
);
self.insert("Option.map", opt_map_ty);

// Option.flatMap   : <a, b>(Option<a>, (a) -> Option<b>) -> Option<b>
let ofm_a = self.fresh_var();
let ofm_b = self.fresh_var();
let opt_flat_map_ty = PolyType::poly(
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
);
self.insert("Option.flatMap", opt_flat_map_ty);

// Option.unwrapOr  : <a>(Option<a>, a) -> a
let uo_a = self.fresh_var();
let unwrap_or_ty = PolyType::poly(
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
);
self.insert("Option.unwrapOr", unwrap_or_ty);
```

**Result methods (2):**
```rust
// Result.map       : <t, e, u>(Result<t, e>, (t) -> u) -> Result<u, e>
let rm_t = self.fresh_var();
let rm_e = self.fresh_var();
let rm_u = self.fresh_var();
let res_map_ty = PolyType::poly(
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
);
self.insert("Result.map", res_map_ty);

// Result.flatMap   : <t, e, u>(Result<t, e>, (t) -> Result<u, e>) -> Result<u, e>
let rfm_t = self.fresh_var();
let rfm_e = self.fresh_var();
let rfm_u = self.fresh_var();
let res_flat_map_ty = PolyType::poly(
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
);
self.insert("Result.flatMap", res_flat_map_ty);
```

**Numeric conversion functions (6):**
```rust
// to_i64 : (i32) -> i64
let to_i64_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::I32]),
    ret: Rc::new(MonoType::I64),
});
self.insert("to_i64", to_i64_ty);

// to_i32 : (f64) -> i32
let to_i32_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::F64]),
    ret: Rc::new(MonoType::I32),
});
self.insert("to_i32", to_i32_ty);

// to_f64 : (i32) -> f64
let to_f64_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::I32]),
    ret: Rc::new(MonoType::F64),
});
self.insert("to_f64", to_f64_ty);

// to_str : (i32) -> str
let to_str_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::I32]),
    ret: Rc::new(MonoType::Str),
});
self.insert("to_str", to_str_ty);

// Also register to_str for i64 and f64:
let to_str_i64_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::I64]),
    ret: Rc::new(MonoType::Str),
});
self.insert("I64.to_str", to_str_i64_ty);

let to_str_f64_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::F64]),
    ret: Rc::new(MonoType::Str),
});
self.insert("F64.to_str", to_str_f64_ty);
```

**Also needed: `readFile` (not `read_file`)**
```rust
// readFile : (str) -> Result<str, str>
let read_file_ty = PolyType::mono(MonoType::Func {
    params: Rc::from([MonoType::Str]),
    ret: Rc::new(MonoType::Tag {
        name: "Result".into(),
        payload: Rc::from([MonoType::Str, MonoType::Str]),
    }),
});
self.insert("readFile", read_file_ty);
```

### Tests for Task 1

Add these tests in `crates/typechecker/src/env.rs` under the `#[cfg(test)]` module:

```rust
#[test]
fn prelude_has_option_map() {
    let mut env = TypeEnv::new();
    env.load_prelude();
    assert!(env.contains("Option.map"));
}

#[test]
fn prelude_has_map() {
    let mut env = TypeEnv::new();
    env.load_prelude();
    assert!(env.contains("map"));
}

#[test]
fn prelude_has_all_builtin_names() {
    let mut env = TypeEnv::new();
    env.load_prelude();
    for name in &[
        "println", "print", "read_line", "map", "filter", "fold",
        "flatMap", "concat", "prepend", "len", "head", "tail",
        "Str.concat", "Str.len", "Str.split", "Str.trim", "Str.parse_i32",
        "Option.map", "Option.flatMap", "Option.unwrapOr",
        "Result.map", "Result.flatMap",
        "to_i64", "to_i32", "to_f64", "to_str",
    ] {
        assert!(env.contains(name), "missing prelude binding: {name}");
    }
}
```

**Run:** `cargo test --lib typechecker -- prelude`

---

## Task 2: Fix Sum Type Constructor Registration

**File:** `crates/typechecker/src/infer.rs`, function `infer_decl_with_map`, line 798

**Current behavior:**
```rust
Decl::TypeAlias { name, .. } => {
    let poly = PolyType::mono(MonoType::Unit);  // <-- IGNORES RHS
    env.insert(*name, poly.clone());
    Ok(poly)
}
```

**Required behavior:** Parse the RHS. For each variant in a sum type, register a constructor function. Populate `tag_variants`. For simple aliases (e.g., `type UserId = i32`), register the alias type.

**Implementation:**

```rust
Decl::TypeAlias { name, params, rhs, span } => {
    // 1. Create quantified type variables for generic params
    let param_vars: Vec<TypeId> = params.iter().map(|_| env.fresh_var()).collect();
    let param_map: HashMap<&str, MonoType> = params.iter()
        .zip(param_vars.iter())
        .map(|(p, &v)| (*p, MonoType::Var(v)))
        .collect();

    // 2. Resolve RHS into a MonoType, handling generic param names
    let resolved_rhs = resolve_type_expr_with_params(env, rhs, &param_map)?;

    // 3. Register the type name itself
    let poly = PolyType::poly(param_vars.clone(), resolved_rhs.clone());
    env.insert(*name, poly);

    // 4. If RHS is a Sum type, register variant constructors
    if let TypeExpr::Sum { variants, .. } = rhs {
        let mut tag_info = Vec::new();

        for variant in variants {
            // Build payload types, replacing generic params with type vars
            let payload_tys: Result<Vec<MonoType>, _> = variant.fields.iter()
                .map(|f| resolve_type_expr_with_params(env, f, &param_map))
                .collect();
            let payload_tys = payload_tys?;

            // Build constructor function type: (payload_tys...) -> TagType
            let constructor_ty = if payload_tys.is_empty() {
                // Nullary constructor (e.g., None, True): bare tag value
                PolyType::poly(
                    param_vars.clone(),
                    MonoType::Tag {
                        name: SmolStr::from(*name),
                        payload: Rc::from([]),
                    },
                )
            } else {
                // Constructor with payload: (T1, T2, ...) -> TagType<...>
                PolyType::poly(
                    param_vars.clone(),
                    MonoType::Func {
                        params: Rc::from(payload_tys.clone()),
                        ret: Rc::new(MonoType::Tag {
                            name: SmolStr::from(*name),
                            payload: Rc::from(payload_tys.clone()),
                        }),
                    },
                )
            };

            env.insert(variant.name, constructor_ty);
            tag_info.push((SmolStr::from(variant.name), payload_tys));
        }

        env.tag_variants.insert(SmolStr::from(*name), tag_info);
    }

    Ok(PolyType::mono(resolved_rhs))
}
```

**New helper function needed:**

```rust
/// Resolve a TypeExpr to MonoType, substituting generic param names with their type vars.
fn resolve_type_expr_with_params(
    env: &TypeEnv,
    te: &TypeExpr<'_>,
    params: &HashMap<&str, MonoType>,
) -> Result<MonoType, TypeError> {
    match te {
        TypeExpr::Named(name, span) => {
            if let Some(subst) = params.get(name) {
                return Ok(subst.clone());
            }
            // Fall through to existing resolution (with env added)
            match *name {
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                | "usize" | "f32" | "f64" | "bool" | "str" | "()" => {
                    type_expr_to_mono_with_env(env, te)
                }
                _ => {
                    // Check env for user-defined types
                    if let Some(poly) = env.lookup(name) {
                        Ok(poly.body.clone())
                    } else {
                        Err(TypeError::UnboundVariable {
                            name: (*name).to_string(),
                            span: *span,
                        })
                    }
                }
            }
        }
        TypeExpr::Apply { func, arg, span } => {
            // Existing Apply logic but needs env for inner resolution
            type_expr_to_mono_with_env(env, te)
        }
        TypeExpr::Function { from, to, .. } => {
            Ok(MonoType::Func {
                params: Rc::from([resolve_type_expr_with_params(env, from, params)?]),
                ret: Rc::new(resolve_type_expr_with_params(env, to, params)?),
            })
        }
        TypeExpr::Tuple { types, .. } => {
            let resolved: Result<Vec<MonoType>, _> = types.iter()
                .map(|t| resolve_type_expr_with_params(env, t, params))
                .collect();
            let payload = Rc::from(resolved?.as_slice());
            Ok(MonoType::Tag {
                name: "Tuple".into(),
                payload,
            })
        }
        TypeExpr::Record { fields, .. } => {
            let mut map = BTreeMap::new();
            for f in fields {
                map.insert(f.name.into(), resolve_type_expr_with_params(env, f.ty, params)?);
            }
            Ok(MonoType::Record(Rc::new(map)))
        }
        TypeExpr::Sum { .. } => Err(TypeError::UnboundVariable {
            name: "anonymous sum type not allowed here".to_string(),
            span: te.span(),
        }),
    }
}
```

### Tests for Task 2

In `crates/typechecker/src/infer.rs`, add:

```rust
#[test]
fn type_alias_simple_named() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    let rhs = bump.alloc(TypeExpr::Named("i32", sp()));
    let decl = Decl::TypeAlias { name: "MyInt", params: bumpalo::collections::Vec::new_in(&bump), rhs, span: sp() };
    let ty = infer_decl(&mut env, &decl).unwrap();
    assert_eq!(ty.body, MonoType::I32);
    assert!(env.contains("MyInt"));
}

#[test]
fn type_alias_sum_constructors() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    // type Option2 = | Some(i32) | None
    let v1 = bump.alloc(TypeVariant { name: "Some", fields: bumpalo::collections::Vec::from_iter_in(
        [bump.alloc(TypeExpr::Named("i32", sp()))], &bump,
    ), span: sp() });
    let v2 = bump.alloc(TypeVariant { name: "None", fields: bumpalo::collections::Vec::new_in(&bump), span: sp() });
    let rhs = bump.alloc(TypeExpr::Sum { variants: bumpalo::collections::Vec::from_iter_in([&*v1, &*v2], &bump), span: sp() });
    let decl = Decl::TypeAlias { name: "Opt", params: bumpalo::collections::Vec::new_in(&bump), rhs, span: sp() };
    infer_decl(&mut env, &decl).unwrap();
    assert!(env.contains("Opt"));
    assert!(env.contains("Some"));
    assert!(env.contains("None"));
    // Some should be: (i32) -> Opt
    let some_ty = env.lookup("Some").unwrap();
    assert_eq!(some_ty.body, MonoType::Func {
        params: Rc::from([MonoType::I32]),
        ret: Rc::new(MonoType::Tag { name: "Opt".into(), payload: Rc::from([MonoType::I32]) }),
    });
}

#[test]
fn type_alias_record() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    // type Person = { name: str, age: i32 }
    let field1 = TypeField { name: "name", ty: bump.alloc(TypeExpr::Named("str", sp())) };
    let field2 = TypeField { name: "age", ty: bump.alloc(TypeExpr::Named("i32", sp())) };
    let rhs = bump.alloc(TypeExpr::Record {
        fields: bumpalo::collections::Vec::from_iter_in([field1, field2], &bump),
        span: sp(),
    });
    let decl = Decl::TypeAlias { name: "Person", params: bumpalo::collections::Vec::new_in(&bump), rhs, span: sp() };
    let ty = infer_decl(&mut env, &decl).unwrap();
    assert_eq!(ty.body, MonoType::Record(Rc::new(BTreeMap::from([
        ("name".into(), MonoType::Str),
        ("age".into(), MonoType::I32),
    ]))));
}
```

**Run:** `cargo test --lib typechecker -- type_alias`

---

## Task 3: Fix `type_expr_to_mono` — Resolve User-Defined Types

**File:** `crates/typechecker/src/infer.rs`, function `type_expr_to_mono`, line 104

### Step 3a: Rename to `type_expr_to_mono_with_env` and add `TypeEnv` parameter

```rust
pub fn type_expr_to_mono_with_env(
    env: &TypeEnv,
    te: &TypeExpr<'_>,
) -> Result<MonoType, TypeError> {
    match te {
        TypeExpr::Named(name, span) => match *name {
            "i8" => Ok(MonoType::I8),
            "i16" => Ok(MonoType::I16),
            "i32" => Ok(MonoType::I32),
            "i64" => Ok(MonoType::I64),
            "u8" => Ok(MonoType::U8),
            "u16" => Ok(MonoType::U16),
            "u32" => Ok(MonoType::U32),
            "u64" => Ok(MonoType::U64),
            "usize" => Ok(MonoType::Usize),
            "f32" => Ok(MonoType::F32),
            "f64" => Ok(MonoType::F64),
            "bool" => Ok(MonoType::Bool),
            "str" => Ok(MonoType::Str),
            "()" => Ok(MonoType::Unit),
            _ => {
                // NEW: Try to resolve user-defined type from env
                if let Some(poly) = env.lookup(name) {
                    // Return the body type (it may be polymorphic — caller handles instantiation)
                    Ok(poly.body.clone())
                } else {
                    Err(TypeError::UnboundVariable {
                        name: (*name).to_string(),
                        span: *span,
                    })
                }
            }
        },
        TypeExpr::Function { from, to, .. } => {
            // NEW: Multi-param fix - if `from` is a Tuple, unwrap it into multiple params
            let from_resolved = type_expr_to_mono_with_env(env, from)?;
            let params = match &from_resolved {
                MonoType::Tag { name, payload } if name.as_str() == "Tuple" => {
                    payload.to_vec()
                }
                _ => vec![from_resolved],
            };
            Ok(MonoType::Func {
                params: Rc::from(params),
                ret: Rc::new(type_expr_to_mono_with_env(env, to)?),
            })
        }
        TypeExpr::Tuple { types, span: _ } => {
            let payload: Result<Vec<MonoType>, _> =
                types.iter().map(|t| type_expr_to_mono_with_env(env, t)).collect();
            Ok(MonoType::Tag {
                name: "Tuple".into(),
                payload: Rc::from(payload?.as_slice()),
            })
        }
        TypeExpr::Record { fields, .. } => {
            let mut map = BTreeMap::new();
            for f in fields {
                map.insert(f.name.into(), type_expr_to_mono_with_env(env, f.ty)?);
            }
            Ok(MonoType::Record(Rc::new(map)))
        }
        TypeExpr::Apply { func, arg, span } => {
            let mut args = vec![type_expr_to_mono_with_env(env, arg)?];
            let mut current = func;
            let base_name = loop {
                match current {
                    TypeExpr::Named(n, _) => break *n,
                    TypeExpr::Apply { func: inner, arg: inner_arg, .. } => {
                        args.push(type_expr_to_mono_with_env(env, inner_arg)?);
                        current = inner;
                    }
                    _ => {
                        return Err(TypeError::UnboundVariable {
                            name: "complex type application".to_string(),
                            span: *span,
                        });
                    }
                }
            };
            args.reverse();
            Ok(MonoType::Tag {
                name: base_name.into(),
                payload: Rc::from(args.as_slice()),
            })
        }
        TypeExpr::Sum { span, .. } => Err(TypeError::UnboundVariable {
            name: "anonymous sum type".to_string(),
            span: *span,
        }),
    }
}
```

### Step 3b: Keep old `type_expr_to_mono` as thin wrapper for backward compat

```rust
/// Legacy wrapper — panics if a user-defined type is encountered.
/// Prefer `type_expr_to_mono_with_env` going forward.
pub fn type_expr_to_mono(te: &TypeExpr<'_>) -> Result<MonoType, TypeError> {
    // Create empty env — will fail on user types
    let empty_env = TypeEnv::new();
    type_expr_to_mono_with_env(&empty_env, te)
}
```

### Step 3c: Update call sites

**In `infer_decl_with_map` (line 777–778):**
```rust
// Before:
let ann_ty = type_expr_to_mono(ann)?;
// After:
let ann_ty = type_expr_to_mono_with_env(env, ann)?;
```

**In `forward_declare_top_level` (lines 80, 92):**
```rust
// Before:
if let Some(ann) = annotation && let Ok(mono) = infer::type_expr_to_mono(ann) {
// After:
if let Some(ann) = annotation && let Ok(mono) = infer::type_expr_to_mono_with_env(env, ann) {
```

```rust
// Before:
infer::type_expr_to_mono(ann).unwrap_or(MonoType::Var(env.fresh_var()))
// After:
infer::type_expr_to_mono_with_env(env, ann).unwrap_or(MonoType::Var(env.fresh_var()))
```

### Tests for Task 3

```rust
#[test]
fn type_annotation_with_user_type() {
    let bump = Bump::new();
    let mut env = TypeEnv::new();
    // First define a type alias
    let rhs = bump.alloc(TypeExpr::Named("i32", sp()));
    let alias = Decl::TypeAlias { name: "MyInt", params: bumpalo::collections::Vec::new_in(&bump), rhs, span: sp() };
    infer_decl(&mut env, &alias).unwrap();
    // Now use it in a let with annotation
    let val = Expr::int("42", sp(), &bump);
    let ann = bump.alloc(TypeExpr::Named("MyInt", sp()));
    let decl = Decl::Bind { name: "x", ty: Some(ann), value: val, span: sp() };
    let ty = infer_decl(&mut env, &decl).unwrap();
    assert_eq!(ty.body, MonoType::I32);
}

#[test]
fn multi_param_function_type_annotation() {
    let bump = Bump::new();
    // (i32, str) -> bool should be 2 params, not 1 tuple param
    let i32 = bump.alloc(TypeExpr::Named("i32", sp()));
    let str = bump.alloc(TypeExpr::Named("str", sp()));
    let tuple = bump.alloc(TypeExpr::Tuple {
        types: bumpalo::collections::Vec::from_iter_in([&*i32, &*str], &bump),
        span: sp(),
    });
    let ret = bump.alloc(TypeExpr::Named("bool", sp()));
    let fn_type = TypeExpr::Function { from: tuple, to: ret, span: sp() };
    let mut env = TypeEnv::new();
    let mono = type_expr_to_mono_with_env(&env, &fn_type).unwrap();
    match mono {
        MonoType::Func { params, ret } => {
            assert_eq!(params.len(), 2, "should have 2 params, got {}", params.len());
            assert_eq!(params[0], MonoType::I32);
            assert_eq!(params[1], MonoType::Str);
            assert_eq!(*ret, MonoType::Bool);
        }
        _ => panic!("expected Func type"),
    }
}

#[test]
fn single_param_function_type_annotation_still_works() {
    let bump = Bump::new();
    // i32 -> bool should still be 1 param
    let from = bump.alloc(TypeExpr::Named("i32", sp()));
    let to = bump.alloc(TypeExpr::Named("bool", sp()));
    let fn_type = TypeExpr::Function { from, to, span: sp() };
    let mut env = TypeEnv::new();
    let mono = type_expr_to_mono_with_env(&env, &fn_type).unwrap();
    match mono {
        MonoType::Func { params, ret } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0], MonoType::I32);
            assert_eq!(*ret, MonoType::Bool);
        }
        _ => panic!("expected Func type"),
    }
}
```

**Run:** `cargo test --lib typechecker -- type_annotation`

---

## Task 4: Fix Multi-Param Type Annotation → IR Lowerer

**File:** `crates/ir/src/lower.rs`

**Root cause:** When the parser parses `(str, i32) -> str`, it produces `Function { from: Tuple([Str, I32]), to: Str }`. The typechecker (after Task 3b's fix) now correctly produces `Func { params: [Str, I32], ret: Str }`. But the IR lowerer may still treat this as 1 param if it looks at the `TypeExpr` directly rather than the resolved `MonoType`.

**Verification:** Check if the lowerer works purely from the type_map (which has the correct `MonoType`) or if it reads `TypeExpr` nodes directly. If it uses type_map, no change needed. If it reads AST type annotations, fix accordingly.

**Search `lower.rs` for direct reading of type annotations:**

Look for `ty: annotation` or `return_type` references in `lower.rs`. If found, ensure they account for multi-param:

```rust
// Before (if applicable):
let param_count = 1; // always 1

// After:
let param_count = match &fn_type {
    MonoType::Func { params, .. } => params.len(),
    _ => 1,
};
```

### Test

```rust
// In IR tests, verify that (i32, str) -> bool lowers to a function with 2 params
```

---

## File: `crates/typechecker/src/lib.rs` — Update `typecheck()` entry point

**The `typecheck()` function calls `infer::infer_decl_with_map` which internally calls `type_expr_to_mono`. Now `type_expr_to_mono_with_env` is used, passing `&env` for type resolution.**

No structural change needed to `typecheck()` — all changes are in `infer.rs` and `env.rs`.

---

## File: `crates/typechecker/src/env.rs` — Export `TypeEnv::lookup` accessors if needed

The function `type_expr_to_mono_with_env` needs access to `env.lookup(name)` which is already public. No change needed.

---

## Task 5: Fix Match-Lowering Constant Type Dispatch (Gap G1)

**File:** `crates/ir/src/lower.rs`, line 504

**Root cause:** Pattern-match lowering emits `Instruction::ConstI64(disc)` for every literal pattern arm, regardless of the subject type. When the subject is `i32`, the `Eq` instruction mixes `i32` and `i64` types, which Cranelift's verifier rejects.

**Fix:** Dispatch on `subj_ty` (already computed at line 428) to emit the correct Const variant:

```rust
let lit_v = fb.emit(match subj_ty {
    IrType::I8 => Instruction::ConstI8(disc as i8),
    IrType::I16 => Instruction::ConstI16(disc as i16),
    IrType::I32 => Instruction::ConstI32(disc as i32),
    IrType::I64 => Instruction::ConstI64(disc),
    IrType::U8 => Instruction::ConstU8(disc as u8),
    IrType::U16 => Instruction::ConstU16(disc as u16),
    IrType::U32 => Instruction::ConstU32(disc as u32),
    IrType::U64 => Instruction::ConstU64(disc as u64),
    IrType::Usize => Instruction::ConstUsize(disc as usize),
    IrType::Bool => Instruction::ConstBool(disc != 0),
    _ => Instruction::ConstI64(disc),
});
```

### Tests

```rust
#[test]
fn match_i32_literal_dispatch_uses_i32_const() {
    // Lower `match x { 0 => 1, _ => 2 }` where x : i32
    // Verify ConstI32, not ConstI64
}
```

## Task 6: Add `Span::contains()` (Gap G2)

**File:** `crates/ast/src/span.rs`

**Required by:** Member 3 LSP hover feature. Adds a simple method:

```rust
/// Returns true if `byte_offset` falls within this span.
pub fn contains(&self, byte_offset: usize) -> bool {
    self.start <= byte_offset && byte_offset < self.end
}
```

## Task 7: Add Effect<T> to the Type System (Critical — Core Pillar)

**Files:** `crates/typechecker/src/types.rs`, `crates/typechecker/src/infer.rs`, `crates/typechecker/src/unify.rs`

### 7a. Add `MonoType::Effect(Box<MonoType>)` to `types.rs`

```rust
pub enum MonoType {
    // ... existing ...
    Effect(Box<MonoType>),
}
```

Update all match arms:
- `Display` → `write!(f, "Effect<{inner}>")`
- `is_concrete` → `Effect(inner) => inner.is_concrete()`
- `free_vars_mono` → traverse inner

### 7b. Update unify.rs

- `Substitution::apply`: `MonoType::Effect(inner) → Box::new(self.apply(inner))`
- `occurs_in`: traverse inner
- `unify`: `(Effect(a), Effect(b)) → unify(sub, a, b)`

### 7c. Handle `Effect<T>` in type annotations

In `type_expr_to_mono_with_env`, the Apply handler already special-cases `"Array"`. Add `"Effect"`:

```rust
"Effect" => Ok(MonoType::Effect(Box::new(args.into_iter().next().unwrap()))),
```

## Task 8: Update Prelude with Effect Signatures

**File:** `crates/typechecker/src/env.rs`

Change IO builtins to return `Effect<T>`:

```rust
// Before:
println : (str) -> ()
// After:
println : (str) -> Effect<()>
print   : (str) -> Effect<()>
readLine : () -> Effect<str>
readFile : (str) -> Effect<Result<str, str>>
```

Add Effect combinators to prelude:

```rust
// Effect.map : <a, b>(Effect<a>, (a) -> b) -> Effect<b>
// Effect.flatMap : <a, b>(Effect<a>, (a) -> Effect<b>) -> Effect<b>
```

## Task 9: Add IrType::Effect + Update IR Lowerer

**Files:** `crates/ir/src/lib.rs`, `crates/ir/src/lower.rs`

### 9a. Add to IrType enum

```rust
Effect(Box<IrType>),
```

- `Display` → `write!(f, "Effect<{inner}>")`
- `is_heap()` → include `IrType::Effect(_)` (behind Arc)

### 9b. Update mono_to_ir_inner

```rust
MonoType::Effect(inner) => IrType::Effect(Box::new(mono_to_ir_inner(inner, tag_variants))),
```

## Task 10: Runtime Effect Support

**Files:** `crates/runtime/src/jit.rs`, `crates/runtime/src/bridge.rs` (or stdlib io.rs)

### 10a. ir_type_tag

Add `IrType::Effect(_) => Some(15)` to `ir_type_tag` in `jit.rs`.

### 10b. IO builtins return Effect (stdlib changes)

Modify `println`, `print`, `readLine`, `readFile` builtins to return `Value::Effect(Arc::new(...))` instead of executing immediately. The Effect wraps the operation as a deferred computation.

### 10c. call_main() executes Effect

In `call_main()`, after calling the main function pointer, check the return tag. If it's `Effect` (tag 15), call `.execute()` on the wrapped `BuiltinFunction`.

```rust
if tag == 15 {
    // Effect — execute it
    let effect: Value = /* reconstruct from ret_buf */;
    if let Value::Effect(func) = effect {
        func.execute(&[])?;
    }
    return Ok(0);
}
```

## Example Program Updates

All example programs with `main` that calls IO functions already work correctly because the typechecker infers `main : () -> Effect<()>` from `println` returning `Effect<()>`. The `hello.pp` program is unchanged.

For `io-effects.pp`, the `readLine().flatMap(...)` chain needs `Effect.flatMap` which is now in the prelude.

## Status of Other Gaps

- **G3 (prelude sigs)** ✅ Already done — `readLine`, `drop`, `take`, `sqrt`, `unwrap` all registered in `env.rs`
- **G4 (JitError::RuntimeError)** → Member 1's scope (added to member1-phase2.md)
- **G5 (From<LowerError>)** → Member 3's scope (added to member3-phase2.md)
- **G6 (From<RuntimeError>)** → Member 3's scope (added to member3-phase2.md)
- **G7 (multi-param IR lowerer)** ✅ Already done — lowerer reads from `type_map`, not `TypeExpr` directly. The multi-param unwrap in `type_expr_to_mono_with_env` (Task 3b) was sufficient.

## Verification (Post-Fix Checklist)

Run these commands in order:

```bash
# 1. Typechecker unit tests
cargo test --lib typechecker

# 2. All example programs typecheck
cargo run -- check example-programs/hello.pp
cargo run -- check example-programs/factorial.pp
cargo run -- check example-programs/fibonacci.pp
cargo run -- check example-programs/sorting.pp
cargo run -- check example-programs/patterns.pp
cargo run -- check example-programs/state-machine.pp
cargo run -- check example-programs/closures.pp
cargo run -- check example-programs/higher-order.pp
cargo run -- check example-programs/records.pp
cargo run -- check example-programs/option-result.pp
cargo run -- check example-programs/io-effects.pp
cargo run -- check example-programs/ascii-art.pp
cargo run -- check example-programs/generics.pp
cargo run -- check example-programs/game-of-life.pp

# 3. All example programs should now pass typecheck (exit code 0)
# Expected: All 14 print no errors and exit with 0

# 4. Full test suite still passes
cargo test --workspace

# 5. Clippy
cargo clippy -- -D warnings

# 6. Format
cargo fmt --check
```

After these fixes, programs will typecheck but may still not **run** — that's what Member 1 Phase 2 is for (JIT builtin bridge + heap ops).

---

## Timeline & Deliverables

| Time | Task | Deliverable |
|---|---|---|
| Hour 0–2 | Task 1: Register 27 prelude type sigs | `env.rs` — all builtins registered + 3 tests |
| Hour 2–4 | Task 2: Fix TypeAlias handler | `infer.rs` — constructor registration + tag_variants + helper + 3 tests |
| Hour 4–6 | Task 3: Fix type_expr_to_mono | `infer.rs` — with_env variant + call site updates + 2 tests |
| Hour 6–8 | Task 4: Fix multi-param IR lowerer | `lower.rs` — param count fix + 1 test |
| Hour 8–10 | Integration & bugfix | All 14 programs typecheck |
| Hour 10–12 | Test pass & verification | Full `cargo test --workspace` passes |

---

## Critical Notes

1. **Do NOT add `Effect<T>` to MonoType** in this phase. The Effect type is deferred to Phase 3. `println` types as `(str) -> ()` for now, not `(str) -> Effect<()>`.

2. **Do NOT change `type_expr_to_mono` in tests** that don't need it. The old `type_expr_to_mono` wrapper (without env) keeps existing tests passing.

3. **The `resolve_type_expr_with_params` helper in Task 2** should be a module-level private function, not a method on `TypeEnv`. It takes `&TypeEnv` + `&HashMap<&str, MonoType>`.

4. **After fixing, verify the example programs actually typecheck** by running `cargo run -- check example-programs/*.pp` — don't just rely on unit tests.

5. **Clippy warnings from the old `type_expr_to_mono` wrapper**: If clippy warns about the unused env when calling with `TypeEnv::new()`, add `#[allow(clippy::needless_pass_by_value)]` or just keep a minimal wrapper that doesn't use env.
