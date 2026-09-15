pub mod ascii;
pub mod binary;

use std::collections::VecDeque;

use crate::{
    fsm::{FSM, latch::LatchOutput},
    value::Value,
    variable::Var,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AigerType {
    Ascii,
    Binary,
}

/// Split the line on spaces and drop empty strings
fn split_line(line: &str) -> Vec<&str> {
    line.split(' ')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
}

/// Reads a line from binary contents and returns it as a String
fn read_binary_line(contents: &mut VecDeque<u8>) -> String {
    let mut line = Vec::new();
    while let Some(c) = contents.pop_front() {
        if c == b'\n' {
            break;
        }
        line.push(c);
    }
    String::from_utf8_lossy(&line).to_string()
}

/// Reads symbols from an AIGER file and assigns them to the FSM (can be either
/// ASCII or binary)
fn read_symbols(contents: &mut VecDeque<u8>, fsm: &mut FSM) {
    while !contents.is_empty() {
        let line = read_binary_line(contents);
        if line == "c" {
            break;
        }
        let Some((symbol, label)) = line.split_once(' ') else {
            continue;
        };
        if label.is_empty() {
            continue;
        }
        let (kind, index) = symbol.split_at(1);
        let index = index.parse::<usize>().unwrap();
        let label = Some(label.to_string());
        match kind {
            "i" => {
                let variable = fsm.get_inputs()[index];
                *fsm.get_variable_label_mut(variable.index()) = label;
            }
            "l" => {
                let variable = fsm.get_latches()[index].output;
                *fsm.get_variable_label_mut(variable.index()) = label;
            }
            "o" => *fsm.get_output_label_mut(index) = label,
            "b" => *fsm.get_assert_label_mut(index) = label,
            "c" => *fsm.get_assume_label_mut(index) = label,
            "j" | "f" => {}
            _ => panic!("unknown AIGER symbol kind {kind}"),
        }
    }
}

/// Writes symbols from the FSM to an AIGER file (can be either ASCII or binary)
fn write_symbols(w: &mut impl std::io::Write, fsm: &FSM) -> std::io::Result<()> {
    for (index, input) in fsm.get_inputs().into_iter().enumerate() {
        if let Some(label) = fsm.get_variable_label(input.index()) {
            writeln!(w, "i{index} {label}")?;
        }
    }
    for (index, latch) in fsm.get_latches().into_iter().enumerate() {
        if let Some(label) = fsm.get_variable_label(latch.output.index()) {
            writeln!(w, "l{index} {label}")?;
        }
    }
    for (index, (_, label)) in fsm.get_outputs().into_iter().enumerate() {
        if let Some(label) = label {
            writeln!(w, "o{index} {label}")?;
        }
    }
    for (index, (_, label)) in fsm.get_asserts().into_iter().enumerate() {
        if let Some(label) = label {
            writeln!(w, "b{index} {label}")?;
        }
    }
    for (index, (_, label)) in fsm.get_assumes().into_iter().enumerate() {
        if let Some(label) = label {
            writeln!(w, "c{index} {label}")?;
        }
    }
    Ok(())
}

/// Converts a usize to a variable
fn into_var(value: usize) -> Var {
    assert!(value != 0 && value != 1);
    let is_negative = (value & 1) == 1;
    let variable_abs = value >> 1;
    Var::from(if is_negative {
        -(variable_abs as i32)
    } else {
        variable_abs as i32
    })
}

/// Converts a var to a usize
fn from_var(variable: Var) -> usize {
    if variable.sign() {
        (variable.value() as usize) * 2
    } else {
        ((-variable.value()) as usize) * 2 + 1
    }
}

/// Converts a usize to a value
fn into_value(value: usize) -> Value {
    if value == 0 {
        Value::Constant(false)
    } else if value == 1 {
        Value::Constant(true)
    } else {
        Value::Var(into_var(value))
    }
}

/// Converts a value to a usize
fn from_value(value: Value) -> usize {
    match value {
        Value::Constant(false) => 0,
        Value::Constant(true) => 1,
        Value::Var(v) => from_var(v),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AigerVersion {
    V1_0,
    V1_9,
}

struct AigerHeader {
    version: AigerVersion,
    num_variables: usize,
    num_inputs: usize,
    num_latches: usize,
    num_outputs: usize,
    num_ands: usize,
    num_assert: usize,
    num_assume: usize,
    num_justice: usize,
    num_fairness: usize,
}

fn read_header(line: &str, aiger_type: AigerType) -> AigerHeader {
    let split = split_line(line);
    assert!(split.len() >= 6);
    match aiger_type {
        AigerType::Ascii => assert_eq!(split[0], "aag"),
        AigerType::Binary => assert_eq!(split[0], "aig"),
    }
    AigerHeader {
        version: if split.len() > 6 {
            AigerVersion::V1_9
        } else {
            AigerVersion::V1_0
        },
        num_variables: split[1].parse::<usize>().unwrap(),
        num_inputs: split[2].parse::<usize>().unwrap(),
        num_latches: split[3].parse::<usize>().unwrap(),
        num_outputs: split[4].parse::<usize>().unwrap(),
        num_ands: split[5].parse::<usize>().unwrap(),
        num_assert: if split.len() >= 7 {
            split[6].parse::<usize>().unwrap()
        } else {
            0
        },
        num_assume: if split.len() >= 8 {
            split[7].parse::<usize>().unwrap()
        } else {
            0
        },
        num_justice: if split.len() >= 9 {
            split[8].parse::<usize>().unwrap()
        } else {
            0
        },
        num_fairness: if split.len() >= 10 {
            split[9].parse::<usize>().unwrap()
        } else {
            0
        },
    }
}

fn write_header(
    w: &mut impl std::io::Write,
    fsm: &FSM,
    aiger_type: AigerType,
    version: AigerVersion,
) -> std::io::Result<()> {
    writeln!(
        w,
        "{} {} {} {} {} {}{}",
        match aiger_type {
            AigerType::Ascii => "aag",
            AigerType::Binary => "aig",
        },
        fsm.get_num_variables(),
        fsm.get_inputs().len(),
        fsm.get_latches().len(),
        fsm.get_outputs().len(),
        fsm.get_gates().len(),
        if version == AigerVersion::V1_9 {
            assert!(fsm.get_covers().is_empty());
            if fsm.get_asserts().is_empty() && fsm.get_assumes().is_empty() {
                String::new()
            } else if fsm.get_assumes().is_empty() {
                format!(" {}", fsm.get_asserts().len())
            } else {
                format!(" {} {}", fsm.get_asserts().len(), fsm.get_assumes().len())
            }
            // format!(
            //     " {} {} {} {}",
            //     fsm.get_asserts().len(),
            //     fsm.get_assumes().len(),
            //     0, // justice
            //     0, // fairness
            // )
        } else {
            assert!(fsm.get_asserts().is_empty());
            assert!(fsm.get_assumes().is_empty());
            assert!(fsm.get_covers().is_empty());
            String::new()
        },
    )
}

fn write_latch(
    w: &mut impl std::io::Write,
    latch: LatchOutput,
    aiger_type: AigerType,
    version: AigerVersion,
) -> std::io::Result<()> {
    writeln!(
        w,
        "{}{}{}",
        if aiger_type == AigerType::Ascii {
            format!("{} ", from_var(latch.output))
        } else {
            String::new() // In binary, output variable is implicit
        },
        from_value(latch.input),
        if version == AigerVersion::V1_0 {
            assert_eq!(latch.reset_value, Some(false));
            // Default is zero, can be omitted
            String::new()
        } else if let Some(default) = latch.reset_value {
            if default {
                " 1".to_string() // If default is one, must be indicated
            } else {
                "".to_string() // If default is zero, can be omitted
            }
        } else {
            // If not reset, use output variable to indicate
            format!(" {}", from_var(latch.output))
        }
    )
}

/// Parses a line containing a single AIGER value from binary contents
fn read_value_line(contents: &mut VecDeque<u8>) -> Value {
    let line = read_binary_line(contents);
    let split = split_line(&line);
    assert_eq!(split.len(), 1);
    into_value(split[0].parse::<usize>().unwrap())
}

/// Parses a latch line from binary contents, optionally given the output
/// variable if it is already known.
fn read_latch_line(
    contents: &mut VecDeque<u8>,
    output_variable: Option<Var>,
) -> (Value, Var, Option<bool>) {
    let line = read_binary_line(contents);
    let mut split = split_line(&line);
    split.reverse();
    let output_variable = if let Some(output_variable) = output_variable {
        assert!(split.len() == 1 || split.len() == 2);
        output_variable
    } else {
        assert!(split.len() == 2 || split.len() == 3);
        into_var(split.pop().unwrap().parse::<usize>().unwrap())
    };
    let input_value = into_value(split.pop().unwrap().parse::<usize>().unwrap());
    // Optionally parse if the latch has a default other than zero
    let default_value = if let Some(default) = split.pop() {
        match default.parse::<usize>().unwrap() {
            0 => Some(false),
            1 => Some(true),
            default_var => {
                assert_eq!(into_var(default_var), output_variable);
                None
            }
        }
    } else {
        Some(false)
    };
    (input_value, output_variable, default_value)
}

impl FSM {
    /// In AIGER v1.0, if there is a single output and no assertions,
    /// the output is treated as an assertion (inverted)
    pub fn normalize_aiger_assertions(&mut self, version: AigerVersion) {
        // Add an assertion if there is only one output and no asserts
        if version == AigerVersion::V1_0
            && self.get_outputs().len() == 1
            && self.get_asserts().is_empty()
        {
            // Invert the outputs since assertions are inverted properties
            let label = self.get_output_label(0).clone();
            let assert_index = self.add_assert(!self.get_outputs()[0].0);
            *self.get_assert_label_mut(assert_index) = label;
        }
    }
}
