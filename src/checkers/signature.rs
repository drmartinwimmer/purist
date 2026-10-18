//! Parameterized checkers for function signatures and return types.

use syn::{FnArg, ReceiverKind as SynReceiverKind, ReturnType, Signature, Type};

/// Represents how a method receives its `self` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiverKind {
    /// Consumes self by value: `self` or `mut self`.
    Value,
    /// Borrows self immutably: `&self`.
    Ref,
    /// Borrows self mutably: `&mut self`.
    RefMut,
    /// Static function without a self parameter.
    None,
}

/// Checks the receiver type of a function signature.
pub(crate) fn check_fn_receiver(sig: &Signature) -> ReceiverKind {
    let Some(first_arg) = sig.inputs.first() else {
        return ReceiverKind::None;
    };

    match first_arg {
        FnArg::Receiver(receiver) => match &receiver.kind {
            SynReceiverKind::Reference(_, _, mutability) => {
                if mutability.is_some() {
                    ReceiverKind::RefMut
                } else {
                    ReceiverKind::Ref
                }
            }
            SynReceiverKind::Value => ReceiverKind::Value,
            SynReceiverKind::Typed(_, ty) => {
                if let Type::Reference(r) = ty.as_ref() {
                    if r.mutability.is_some() {
                        ReceiverKind::RefMut
                    } else {
                        ReceiverKind::Ref
                    }
                } else {
                    ReceiverKind::Value
                }
            }
            _ => ReceiverKind::Value,
        },
        FnArg::Typed(pat_type) => {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat
                && pat_ident.ident == "self"
            {
                if let Type::Reference(r) = &*pat_type.ty {
                    if r.mutability.is_some() {
                        ReceiverKind::RefMut
                    } else {
                        ReceiverKind::Ref
                    }
                } else {
                    ReceiverKind::Value
                }
            } else {
                ReceiverKind::None
            }
        }
    }
}

/// Extracts the return type from a function signature, if non-unit.
pub(crate) fn extract_fn_return_type(sig: &Signature) -> Option<&Type> {
    match &sig.output {
        ReturnType::Type(_, ty) => Some(ty.as_ref()),
        ReturnType::Default => None,
    }
}
