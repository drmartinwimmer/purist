//! Parameterized checkers for function signatures and return types.

use syn::{
    FnArg, GenericArgument, PathArguments, ReceiverKind as SynReceiverKind, ReturnType, Signature,
    Type,
};

/// Represents how a method receives its `self` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverKind {
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
pub fn check_fn_receiver(sig: &Signature) -> ReceiverKind {
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
pub fn extract_fn_return_type(sig: &Signature) -> Option<&Type> {
    match &sig.output {
        ReturnType::Type(_, ty) => Some(ty.as_ref()),
        ReturnType::Default => None,
    }
}

/// If a type is `Result<T, E>`, extracts the error type `E`.
pub fn extract_result_error_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return None;
    };

    // If Result<T, E>, the error type is the second generic argument
    let second_arg = args.args.iter().nth(1)?;
    match second_arg {
        GenericArgument::Type(error_ty) => Some(error_ty),
        _ => None,
    }
}
