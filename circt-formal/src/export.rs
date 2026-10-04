use formal_utils::{
    formats::aiger::{AigerVersion, ascii::write_aiger_ascii, binary::write_aiger_binary},
    fsm::{FSM, verify::VerifyOrdering},
};

use crate::error::ConvertError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExportOptions {
    pub zero_init: bool,
    pub strip_symbols: bool,
    pub assertion: Option<usize>,
    pub cover: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AigerFormat {
    Ascii,
    Binary,
}

/// Prepare an export copy without invalidating the named model's references.
pub fn prepare(fsm: &FSM, options: &ExportOptions) -> Result<FSM, ConvertError> {
    if options.assertion.is_some() && options.cover.is_some() {
        return Err(ConvertError::message(
            "assertion and cover selection are mutually exclusive",
        ));
    }
    let mut fsm = fsm.clone();
    if let Some(index) = options.assertion {
        let assertion = fsm.get_asserts().get(index).cloned().ok_or_else(|| {
            ConvertError::message(format!(
                "assertion index {index} is out of range (found {})",
                fsm.get_asserts().len()
            ))
        })?;
        fsm.get_asserts_mut().clear();
        fsm.get_asserts_mut().push(assertion);
    }
    if let Some(index) = options.cover {
        let (value, label) = fsm.get_covers().get(index).cloned().ok_or_else(|| {
            ConvertError::message(format!(
                "cover index {index} is out of range (found {})",
                fsm.get_covers().len()
            ))
        })?;
        fsm.get_asserts_mut().clear();
        fsm.get_asserts_mut().push((!value, label));
    }
    fsm.get_covers_mut().clear();
    if options.zero_init {
        for latch in fsm.get_latches() {
            fsm.get_latch_mut(latch.output.index()).unwrap().reset_value = Some(false);
        }
    }
    fsm.normalize_outputs();
    fsm.reorder_gates();
    fsm.verify(VerifyOrdering::Verify);
    // AIGER symbols are line records. Escape control characters and backslashes
    // without changing the named model or merging distinct decoded MLIR names.
    for index in 0..fsm.get_num_variables() {
        if let Some(label) = fsm.get_variable_label_mut(index) {
            *label = escape_symbol(label);
        }
    }
    for (_, label) in fsm.get_outputs_mut() {
        if let Some(label) = label {
            *label = escape_symbol(label);
        }
    }
    for (_, label) in fsm.get_asserts_mut() {
        if let Some(label) = label {
            *label = escape_symbol(label);
        }
    }
    for (_, label) in fsm.get_assumes_mut() {
        if let Some(label) = label {
            *label = escape_symbol(label);
        }
    }
    if options.strip_symbols {
        for index in 0..fsm.get_num_variables() {
            *fsm.get_variable_label_mut(index) = None;
        }
        for (_, label) in fsm.get_outputs_mut() {
            *label = None;
        }
        for (_, label) in fsm.get_asserts_mut() {
            *label = None;
        }
        for (_, label) in fsm.get_assumes_mut() {
            *label = None;
        }
    }
    Ok(fsm)
}

pub fn write(
    fsm: &FSM,
    format: AigerFormat,
    options: &ExportOptions,
) -> Result<Vec<u8>, ConvertError> {
    let fsm = prepare(fsm, options)?;
    Ok(match format {
        AigerFormat::Ascii => write_aiger_ascii(&fsm, AigerVersion::V1_9),
        AigerFormat::Binary => write_aiger_binary(&fsm, AigerVersion::V1_9),
    })
}

pub fn symbols(fsm: &FSM) -> Vec<String> {
    let mut records = Vec::new();
    for (index, input) in fsm.get_inputs().into_iter().enumerate() {
        if let Some(label) = fsm.get_variable_label(input.index()) {
            records.push(format!("i{index} {label}"));
        }
    }
    for (index, latch) in fsm.get_latches().into_iter().enumerate() {
        if let Some(label) = fsm.get_variable_label(latch.output.index()) {
            records.push(format!("l{index} {label}"));
        }
    }
    for (prefix, properties) in [
        ("o", fsm.get_outputs()),
        ("b", fsm.get_asserts()),
        ("c", fsm.get_assumes()),
    ] {
        for (index, (_, label)) in properties.into_iter().enumerate() {
            if let Some(label) = label {
                records.push(format!("{prefix}{index} {label}"));
            }
        }
    }
    records
}

fn escape_symbol(label: &str) -> String {
    let mut escaped = String::new();
    for character in label.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\0' => escaped.push_str("\\0"),
            _ => escaped.push(character),
        }
    }
    escaped
}
