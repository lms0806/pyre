//! W_UnionType — Python `types.UnionType` (PEP 604).
//!
//! PyPy equivalent: lib_pypy/_pypy_generic_alias.py → UnionType
//!
//! Represents `X | Y` union types (e.g. `int | str`).
//! Supports `isinstance`, `issubclass`, deduplication, and flattening.

#![allow(unsafe_op_in_unsafe_fn)]

use crate::pyobject::*;
use pyre_macros::pyre_class;

/// Python union type object (PEP 604).
///
/// Layout: `[ob_type | args | parameters]`
///
/// - `args`: tuple of the union members (deduplicated, flattened)
/// - `parameters`: tuple of free type variables — `__parameters__`
///
/// PyPy equivalent: UnionType in _pypy_generic_alias.py
#[pyre_class("types.UnionType", type_id = 22, static_name = "UNION")]
pub struct W_UnionType {
    /// Tuple of union member types — PyPy: UnionType._args
    pub args: PyObjectRef,
    /// Tuple of free type variables, `_collect_parameters(args)` computed at
    /// construction from the raw constructor operands — PyPy:
    /// `UnionType.__parameters__`.
    pub parameters: PyObjectRef,
}

/// Check if an object is a UnionType.
#[inline]
pub unsafe fn is_union(obj: PyObjectRef) -> bool {
    py_type_check(obj, &UNION_TYPE)
}

/// Allocate a UnionType from already-flattened/deduplicated members and a
/// precomputed parameters tuple — the body of `UnionType.__init__` after
/// `add_recurse`.  `UnionType(())` (the empty-substitution case of
/// `UnionType.__getitem__`) uses this with an empty member list.
pub fn w_union_from_members(members: Vec<PyObjectRef>, parameters: PyObjectRef) -> PyObjectRef {
    let args = crate::w_tuple_new(members);
    // `gct_fv_gc_malloc` bracket pattern (`framework.py:853-856`).
    let _roots = crate::gc_roots::push_roots();
    crate::gc_roots::pin_root(args);
    crate::gc_roots::pin_root(parameters);
    W_UnionType::allocate(W_UnionType {
        ob: PyObject {
            ob_type: std::ptr::null(),
            w_class: std::ptr::null_mut(),
        },
        args,
        parameters,
    })
}

/// Get the `__args__` tuple of a UnionType.
///
/// # Safety
/// `obj` must point to a valid `W_UnionType`.
pub unsafe fn w_union_get_args(obj: PyObjectRef) -> PyObjectRef {
    (*(obj as *const W_UnionType)).args
}

/// Get the `__parameters__` tuple of a UnionType.
///
/// # Safety
/// `obj` must point to a valid `W_UnionType`.
pub unsafe fn w_union_get_parameters(obj: PyObjectRef) -> PyObjectRef {
    (*(obj as *const W_UnionType)).parameters
}

/// Check if `instance` is an instance of any type in the union.
///
/// PyPy equivalent: UnionType.__instancecheck__
pub unsafe fn w_union_instancecheck(union: PyObjectRef, instance: PyObjectRef) -> bool {
    let args = w_union_get_args(union);
    let n = crate::w_tuple_len(args);
    for i in 0..n {
        if let Some(cls) = crate::w_tuple_getitem(args, i as i64) {
            if is_none(cls) {
                if is_none(instance) {
                    return true;
                }
            } else if crate::is_type(cls) {
                // Use ob_type pointer comparison for builtin types
                if std::ptr::eq((*instance).ob_type, (*cls).ob_type)
                    || std::ptr::eq(
                        (*instance).ob_type as *const u8,
                        cls as *const u8 as *const PyType as *const u8,
                    )
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intobject::w_int_new;

    fn no_params() -> PyObjectRef {
        crate::w_tuple_new(vec![])
    }

    #[test]
    fn test_union_create() {
        // Flatten/dedup happens in the interp layer (`create_union` →
        // `add_recurse`, which needs `compare`); this wraps already-prepared
        // members.
        let a = w_int_new(1); // stand-in for int type
        let b = w_int_new(2); // stand-in for str type
        let union = w_union_from_members(vec![a, b], no_params());
        unsafe {
            assert!(is_union(union));
            let args = w_union_get_args(union);
            assert_eq!(crate::w_tuple_len(args), 2);
        }
    }

    #[test]
    fn w_union_gc_type_id_matches_descr() {
        assert_eq!(W_UNION_GC_TYPE_ID, 22);
        assert_eq!(
            <W_UnionType as crate::lltype::GcType>::type_id(),
            W_UNION_GC_TYPE_ID
        );
        assert_eq!(
            <W_UnionType as crate::lltype::GcType>::SIZE,
            W_UNION_OBJECT_SIZE
        );
    }
}
