use std::{collections::BTreeSet, str::FromStr};

use formal_utils::{
    fsm::{FSM, verify::VerifyOrdering},
    gate::GateType,
    ops::{Comparison, FsmOps, ShiftOperation},
    sim::Simulator,
    value::Value,
};
use parser_circt::{
    ast::{
        ClockEdge, CombOperator, ComparisonPredicate, ConstantValue, File, OperationKind,
        PropertyKind, ResetKind,
    },
    parser::parse,
};

use crate::{
    error::ConvertError,
    graph::{Driver, Graph, Ty, decode},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockSpec {
    pub name: String,
    pub edge: ClockEdge,
}

impl FromStr for ClockSpec {
    type Err = ConvertError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (name, negative) = signal_spec(text)?;
        Ok(Self {
            name,
            edge: if negative {
                ClockEdge::Negative
            } else {
                ClockEdge::Positive
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetSpec {
    pub name: String,
    pub active_high: bool,
}

impl FromStr for ResetSpec {
    type Err = ConvertError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (name, negative) = signal_spec(text)?;
        Ok(Self {
            name,
            active_high: !negative,
        })
    }
}

fn signal_spec(text: &str) -> Result<(String, bool), ConvertError> {
    let negative = text.starts_with('!');
    let name = text.strip_prefix('!').unwrap_or(text);
    if name.is_empty() {
        return Err(ConvertError::message("signal name cannot be empty"));
    }
    Ok((name.to_owned(), negative))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversionOptions {
    pub top: Option<String>,
    pub clock: ClockSpec,
    pub reset: Option<ResetSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedBit {
    pub index: isize,
    pub value: Value,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedSignal {
    pub name: String,
    pub bits: Vec<NamedBit>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedProperty {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct NamedFsm {
    pub fsm: FSM,
    pub top: String,
    pub clock: ClockSpec,
    pub reset: Option<ResetSpec>,
    pub inputs: Vec<NamedSignal>,
    pub registers: Vec<NamedSignal>,
    pub outputs: Vec<NamedSignal>,
    pub assertions: Vec<NamedProperty>,
    pub assumptions: Vec<NamedProperty>,
    pub covers: Vec<NamedProperty>,
    /// Conversion diagnostics. Library conversion does not write to stderr.
    pub warnings: Vec<String>,
}

impl NamedFsm {
    pub fn from_ast(file: &File, options: &ConversionOptions) -> Result<Self, ConvertError> {
        let graph = Graph::build(file, options)?;
        Converter::new(graph).convert(options)
    }

    pub fn from_source(
        file_id: usize,
        source: &str,
        options: &ConversionOptions,
    ) -> Result<Self, ConvertError> {
        Self::from_ast(&parse(file_id, source)?, options)
    }

    pub fn initialize_registers_to_zero(&mut self) {
        for latch in self.fsm.get_latches() {
            self.fsm
                .get_latch_mut(latch.output.index())
                .unwrap()
                .reset_value = Some(false);
        }
    }
}

fn signal(name: String, values: &[Value]) -> NamedSignal {
    NamedSignal {
        name,
        bits: values
            .into_iter()
            .enumerate()
            .map(|(index, &value)| NamedBit {
                index: index as isize,
                value,
            })
            .collect(),
    }
}

struct Converter {
    graph: Graph,
    fsm: FSM,
    values: Vec<Option<Vec<Value>>>,
    undefined: Vec<Option<Vec<Value>>>,
}

impl Converter {
    fn new(graph: Graph) -> Self {
        let count = graph.nodes.len();
        Self {
            graph,
            fsm: FSM::default(),
            values: vec![None; count],
            undefined: vec![None; count],
        }
    }

    fn input(&mut self, width: usize, name: &str) -> Vec<Value> {
        (0..width)
            .map(|bit| {
                let var = self.fsm.add_variable_input();
                *self.fsm.get_variable_label_mut(var.index()) = Some(bit_label(name, bit, width));
                Value::from(var)
            })
            .collect()
    }

    fn convert(mut self, options: &ConversionOptions) -> Result<NamedFsm, ConvertError> {
        let mut inputs = Vec::new();
        let mut registers = Vec::new();
        let mut names = BTreeSet::new();
        for id in self.graph.inputs.clone() {
            if id == self.graph.clock {
                continue;
            }
            let node = &self.graph.nodes[id];
            let width = node
                .ty
                .width()
                .map_err(|_| node.error("multiple native clock inputs are unsupported"))?;
            let name = unique(&mut names, &node.name);
            let values = self.input(width, &name);
            inputs.push(signal(name, &values));
            self.values[id] = Some(values);
        }
        // Nondeterminism must also precede every latch/gate in AIGER ordering.
        for id in 0..self.graph.nodes.len() {
            let node = &self.graph.nodes[id];
            let Driver::Op(op, _) = &node.driver else {
                continue;
            };
            let needed = match &op.kind {
                OperationKind::Comb { operator, .. } => [
                    CombOperator::DivSigned,
                    CombOperator::DivUnsigned,
                    CombOperator::ModSigned,
                    CombOperator::ModUnsigned,
                ]
                .contains(operator),
                OperationKind::ArrayGet { array_type, .. }
                | OperationKind::ArrayInject { array_type, .. } => match Ty::parse(array_type)? {
                    Ty::Array(size, _) => size == 1 || !size.is_power_of_two(),
                    _ => false,
                },
                _ => false,
            };
            if needed {
                let name = unique(&mut names, &format!("$undefined::{}", node.name));
                let values = self.input(node.ty.width()?, &name);
                inputs.push(signal(name, &values));
                self.undefined[id] = Some(values);
            }
        }
        let mut async_names = Vec::new();
        for id in 0..self.graph.nodes.len() {
            let node = &self.graph.nodes[id];
            if !node.register() {
                continue;
            }
            let Driver::Op(op, _) = &node.driver else {
                unreachable!()
            };
            let name = match &op.kind {
                OperationKind::FirReg {
                    symbol: Some(symbol),
                    ..
                } => {
                    let local = decode(&symbol.spelling)?;
                    // node.path contains the top; labels omit the top prefix.
                    let path = node
                        .path
                        .strip_prefix(&self.graph.top)
                        .unwrap_or(&node.path)
                        .trim_start_matches("::");
                    if path.is_empty() {
                        local
                    } else {
                        format!("{path}::{local}")
                    }
                }
                OperationKind::CompReg {
                    name: Some(name), ..
                } if name.spelling.as_ref() != b"\"\"" => {
                    let local = decode(&name.spelling)?;
                    let path = node
                        .path
                        .strip_prefix(&self.graph.top)
                        .unwrap_or(&node.path)
                        .trim_start_matches("::");
                    if path.is_empty() {
                        local
                    } else {
                        format!("{path}::{local}")
                    }
                }
                _ => node.name.clone(),
            };
            let name = unique(&mut names, &name);
            let width = node.ty.width()?;
            let values = (0..width)
                .map(|bit| {
                    let var = self.fsm.add_variable();
                    *self.fsm.get_variable_label_mut(var.index()) =
                        Some(bit_label(&name, bit, width));
                    Value::from(var)
                })
                .collect::<Vec<_>>();
            if let OperationKind::FirReg {
                reset: Some(reset), ..
            } = &op.kind
                && reset.kind == ResetKind::Async
            {
                async_names.push(name.clone());
            }
            registers.push(signal(name, &values));
            self.values[id] = Some(values);
        }
        for id in 0..self.graph.nodes.len() {
            if !self.graph.is_clock_wiring(id) {
                self.lower(id)?;
            }
        }
        for id in 0..self.graph.nodes.len() {
            if !self.graph.nodes[id].register() {
                continue;
            }
            let node = self.graph.nodes[id].clone();
            let Driver::Op(op, args) = &node.driver else {
                unreachable!()
            };
            let mut next = self.lower(args[0])?;
            if args.len() == 4 {
                let select = self.lower(args[2])?[0];
                let reset = self.lower(args[3])?;
                next = self.fsm.create_mux(&next, &reset, select);
            }
            let width = next.len();
            let preset = match &op.kind {
                OperationKind::FirReg {
                    preset: Some(preset),
                    ..
                } => {
                    Some(constant(&preset.spelling, width).map_err(|err| node.error(err.message))?)
                }
                _ => None,
            };
            for (bit, (&output, input)) in self.values[id]
                .as_ref()
                .unwrap()
                .into_iter()
                .zip(next)
                .enumerate()
            {
                let var = output.unwrap_variable();
                // add_latch replaces driver and label, so restore the label.
                let label = self.fsm.get_variable_label(var.index()).clone();
                self.fsm.add_latch(
                    input,
                    var,
                    preset
                        .as_ref()
                        .map(|bits| bits[bit] == Value::Constant(true)),
                );
                *self.fsm.get_variable_label_mut(var.index()) = label;
            }
        }
        let mut outputs = Vec::new();
        for (name, id) in self.graph.outputs.clone() {
            let bits = self.lower(id)?;
            for (bit, &value) in (&bits).into_iter().enumerate() {
                let index = self.fsm.add_output(value);
                *self.fsm.get_output_label_mut(index) = Some(bit_label(&name, bit, bits.len()));
            }
            outputs.push(signal(name, &bits));
        }
        let mut assertions = Vec::new();
        let mut assumptions = Vec::new();
        let mut covers = Vec::new();
        let mut property_names = BTreeSet::new();
        let mut ordinals = [0usize; 3];
        for index in 0..self.graph.properties.len() {
            let property = &self.graph.properties[index];
            let OperationKind::ClockedProperty { kind, label, .. } = &property.op.kind else {
                unreachable!()
            };
            let kind = *kind;
            let kind_index = match kind {
                PropertyKind::Assert => 0,
                PropertyKind::Assume => 1,
                PropertyKind::Cover => 2,
            };
            let kind_name = ["assert", "assume", "cover"][kind_index];
            let local = label
                .as_ref()
                .map(|label| decode(&label.spelling))
                .transpose()?
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("{kind_name}[{}]", ordinals[kind_index]));
            ordinals[kind_index] += 1;
            let name = unique(&mut property_names, &format!("{}::{local}", property.path));
            let args = property.args.clone();
            let value = self.lower(args[0])?[0];
            let enable = if args.len() == 3 {
                self.lower(args[1])?[0]
            } else {
                Value::Constant(true)
            };
            let value = if kind == PropertyKind::Cover {
                self.fsm
                    .add_variable_gate(GateType::And, vec![enable, value])
            } else {
                self.fsm
                    .add_variable_gate(GateType::Or, vec![!enable, value])
            };
            let named = NamedProperty {
                name: name.clone(),
                value,
            };
            match kind {
                PropertyKind::Assert => {
                    let index = self.fsm.add_assert(value);
                    *self.fsm.get_assert_label_mut(index) = Some(name);
                    assertions.push(named);
                }
                PropertyKind::Assume => {
                    let index = self.fsm.add_assume(value);
                    *self.fsm.get_assume_label_mut(index) = Some(name);
                    assumptions.push(named);
                }
                PropertyKind::Cover => {
                    let index = self.fsm.add_cover(value);
                    *self.fsm.get_cover_label_mut(index) = Some(name);
                    covers.push(named);
                }
            }
        }
        self.fsm.verify(VerifyOrdering::Verify);
        let warnings = if async_names.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "treating {} asynchronous reset register(s) as synchronous; between-edge reset events are ignored: {}",
                async_names.len(),
                async_names.join(", ")
            )]
        };
        let mut model = NamedFsm {
            fsm: self.fsm,
            top: self.graph.top,
            clock: options.clock.clone(),
            reset: options.reset.clone(),
            inputs,
            registers,
            outputs,
            assertions,
            assumptions,
            covers,
            warnings,
        };
        if let Some(reset) = &options.reset {
            if reset.name == options.clock.name {
                return Err(ConvertError::message(
                    "clock and reset must name different signals",
                ));
            }
            apply_reset(&mut model, reset)?;
        }
        Ok(model)
    }

    fn lower(&mut self, id: usize) -> Result<Vec<Value>, ConvertError> {
        if let Some(value) = &self.values[id] {
            return Ok(value.clone());
        }
        let node = self.graph.nodes[id].clone();
        let value = match &node.driver {
            Driver::Alias(other) => self.lower(*other)?,
            Driver::Op(op, args) => {
                if self.graph.is_clock_wiring(id) {
                    return Err(node.error("clock used as data"));
                }
                let values = args
                    .into_iter()
                    .map(|&arg| self.lower(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                match &op.kind {
                    OperationKind::Constant { value, .. } => match value {
                        ConstantValue::Boolean(value) => vec![Value::Constant(*value)],
                        ConstantValue::Integer(value) => {
                            constant(&value.spelling, node.ty.width()?)
                                .map_err(|err| node.error(err.message))?
                        }
                    },
                    OperationKind::Comb { operator, .. } => {
                        let mut result = values[0].clone();
                        for rhs in &values[1..] {
                            result = match operator {
                                CombOperator::Add => self.fsm.create_addition(&result, rhs),
                                CombOperator::Sub => self.fsm.create_subtraction(&result, rhs),
                                CombOperator::Mul => self.fsm.create_multiplication(&result, rhs),
                                CombOperator::DivUnsigned => {
                                    self.fsm.create_unsigned_division(&result, rhs)
                                }
                                CombOperator::DivSigned => {
                                    self.fsm.create_signed_division(&result, rhs)
                                }
                                CombOperator::ModUnsigned => {
                                    self.fsm.create_unsigned_remainder(&result, rhs)
                                }
                                CombOperator::ModSigned => {
                                    self.fsm.create_signed_remainder(&result, rhs)
                                }
                                CombOperator::And | CombOperator::Or | CombOperator::Xor => result
                                    .into_iter()
                                    .zip(rhs)
                                    .map(|(lhs, &rhs)| match operator {
                                        CombOperator::Xor => self.fsm.create_xor_gate(lhs, rhs),
                                        CombOperator::And => self
                                            .fsm
                                            .add_variable_gate(GateType::And, vec![lhs, rhs]),
                                        _ => {
                                            self.fsm.add_variable_gate(GateType::Or, vec![lhs, rhs])
                                        }
                                    })
                                    .collect(),
                                CombOperator::ShiftLeft
                                | CombOperator::ShiftRightSigned
                                | CombOperator::ShiftRightUnsigned => self.fsm.create_shifter(
                                    &result,
                                    rhs,
                                    match operator {
                                        CombOperator::ShiftLeft => ShiftOperation::ShiftLeft,
                                        CombOperator::ShiftRightSigned => {
                                            ShiftOperation::ShiftRightArithmetic
                                        }
                                        _ => ShiftOperation::ShiftRight,
                                    },
                                ),
                            };
                        }
                        if let Some(undefined) = &self.undefined[id] {
                            let zero = self.fsm.add_variable_gate(
                                GateType::And,
                                values[1].clone().into_iter().map(|value| !value).collect(),
                            );
                            result = self.fsm.create_mux(&result, undefined, zero);
                        }
                        result
                    }
                    OperationKind::Compare { predicate, .. } => {
                        let (comparison, signed) = match predicate {
                            ComparisonPredicate::Equal
                            | ComparisonPredicate::CaseEqual
                            | ComparisonPredicate::WildcardEqual => (Comparison::Equals, false),
                            ComparisonPredicate::NotEqual
                            | ComparisonPredicate::CaseNotEqual
                            | ComparisonPredicate::WildcardNotEqual => {
                                (Comparison::NotEquals, false)
                            }
                            ComparisonPredicate::SignedLess => (Comparison::LessThan, true),
                            ComparisonPredicate::SignedLessEqual => {
                                (Comparison::LessThanOrEqual, true)
                            }
                            ComparisonPredicate::SignedGreater => (Comparison::GreaterThan, true),
                            ComparisonPredicate::SignedGreaterEqual => {
                                (Comparison::GreaterThanOrEqual, true)
                            }
                            ComparisonPredicate::UnsignedLess => (Comparison::LessThan, false),
                            ComparisonPredicate::UnsignedLessEqual => {
                                (Comparison::LessThanOrEqual, false)
                            }
                            ComparisonPredicate::UnsignedGreater => {
                                (Comparison::GreaterThan, false)
                            }
                            ComparisonPredicate::UnsignedGreaterEqual => {
                                (Comparison::GreaterThanOrEqual, false)
                            }
                        };
                        let mut lhs = values[0].clone();
                        let mut rhs = values[1].clone();
                        if signed {
                            let last = lhs.len() - 1;
                            lhs[last] = !lhs[last];
                            rhs[last] = !rhs[last];
                        }
                        vec![self.fsm.create_comparison(&lhs, &rhs, comparison)]
                    }
                    OperationKind::Mux { .. } => {
                        self.fsm.create_mux(&values[2], &values[1], values[0][0])
                    }
                    OperationKind::UnionExtract {
                        union_type, field, ..
                    } => {
                        let ty = Ty::parse(union_type)?;
                        let (member, offset) = ty.union_field(&decode(&field.spelling)?)?;
                        values[0][offset..offset + member.width()?].to_vec()
                    }
                    OperationKind::Bitcast { .. } => values[0].clone(),
                    OperationKind::Concat { .. } => values.into_iter().rev().flatten().collect(),
                    OperationKind::Replicate { .. } => {
                        values[0].repeat(node.ty.width()? / values[0].len())
                    }
                    OperationKind::Extract { offset, .. } => {
                        values[0][*offset as usize..*offset as usize + node.ty.width()?].to_vec()
                    }
                    OperationKind::ArrayGet { array_type, .. }
                    | OperationKind::ArrayInject { array_type, .. } => {
                        let Ty::Array(size, element) = Ty::parse(array_type)? else {
                            unreachable!()
                        };
                        let width = element.width()?;
                        let mut result = if values.len() == 3 {
                            values[0].clone()
                        } else {
                            vec![Value::Constant(false); width]
                        };
                        for index in 0..size {
                            let index_bits = unsigned(index, values[1].len());
                            let select = self.fsm.create_comparison(
                                &values[1],
                                &index_bits,
                                Comparison::Equals,
                            );
                            if values.len() == 3 {
                                let updated = self.fsm.create_mux(
                                    &result[index * width..(index + 1) * width],
                                    &values[2],
                                    select,
                                );
                                result[index * width..(index + 1) * width]
                                    .copy_from_slice(&updated);
                            } else {
                                result = self.fsm.create_mux(
                                    &result,
                                    &values[0][index * width..(index + 1) * width],
                                    select,
                                );
                            }
                        }
                        if let Some(undefined) = &self.undefined[id] {
                            let limit = unsigned(size - 1, values[1].len());
                            let valid = self.fsm.create_comparison(
                                &values[1],
                                &limit,
                                Comparison::LessThanOrEqual,
                            );
                            result = self.fsm.create_mux(undefined, &result, valid);
                        }
                        result
                    }
                    _ => return Err(node.error("unsupported data operation")),
                }
            }
            _ => return Err(node.error("unallocated input/register")),
        };
        self.values[id] = Some(value.clone());
        Ok(value)
    }
}

fn bit_label(name: &str, bit: usize, width: usize) -> String {
    if width == 1 {
        name.to_owned()
    } else {
        format!("{name}[{bit}]")
    }
}
fn unique(names: &mut BTreeSet<String>, base: &str) -> String {
    let mut result = base.to_owned();
    let mut suffix = 1;
    while !names.insert(result.clone()) {
        result = format!("{base}${suffix}");
        suffix += 1;
    }
    result
}
fn unsigned(value: usize, width: usize) -> Vec<Value> {
    (0..width)
        .map(|bit| Value::Constant(bit < usize::BITS as usize && value & (1usize << bit) != 0))
        .collect()
}

/// Accumulate modulo 2^width without constraining constants to a host integer.
fn constant(spelling: &[u8], width: usize) -> Result<Vec<Value>, ConvertError> {
    let text = std::str::from_utf8(spelling)
        .map_err(|_| ConvertError::message("invalid integer literal"))?;
    let negative = text.starts_with('-');
    let text = text
        .strip_prefix('-')
        .or_else(|| text.strip_prefix('+'))
        .unwrap_or(text);
    let (digits, radix) =
        if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (digits, 16usize)
        } else {
            (text, 10usize)
        };
    if digits.is_empty() {
        return Err(ConvertError::message("empty integer literal"));
    }
    let mut bits = vec![false; width];
    for digit in digits.chars() {
        let mut carry = digit
            .to_digit(radix as u32)
            .ok_or_else(|| ConvertError::message("invalid integer literal digit"))?
            as usize;
        for bit in &mut bits {
            let value = usize::from(*bit) * radix + carry;
            *bit = value & 1 != 0;
            carry = value >> 1;
        }
    }
    if negative {
        let mut carry = true;
        for bit in &mut bits {
            let inverted = !*bit;
            *bit = inverted ^ carry;
            carry &= inverted;
        }
    }
    Ok(bits.into_iter().map(Value::Constant).collect())
}

fn remap(value: Value, mapping: &[Value]) -> Value {
    value.map_variable_value(|var| mapping[var.index()].xor(!var.sign()))
}

fn apply_reset(model: &mut NamedFsm, reset: &ResetSpec) -> Result<(), ConvertError> {
    let signal = (&model.inputs)
        .into_iter()
        .find(|signal| signal.name == reset.name)
        .ok_or_else(|| ConvertError::message(format!("unknown reset input {}", reset.name)))?;
    if signal.bits.len() != 1 {
        return Err(ConvertError::message("reset input must be i1"));
    }
    let tied = signal.bits[0].value.unwrap_variable().index();
    let mut simulator = Simulator::from(model.fsm.clone());
    simulator.set_input(tied, Some(reset.active_high));
    simulator.eval();
    simulator.step();
    for latch in model.fsm.get_latches() {
        model
            .fsm
            .get_latch_mut(latch.output.index())
            .unwrap()
            .reset_value = simulator
            .get_variable_signed(latch.output)
            .or(latch.reset_value);
    }
    let old = &model.fsm;
    let mut rebuilt = FSM::default();
    let mut mapping = vec![Value::Constant(false); old.get_num_variables()];
    mapping[tied] = Value::Constant(!reset.active_high);
    for input in old.get_inputs() {
        if input.index() == tied {
            continue;
        }
        let var = rebuilt.add_variable_input();
        mapping[input.index()] = Value::from(var);
        *rebuilt.get_variable_label_mut(var.index()) =
            old.get_variable_label(input.index()).clone();
    }
    for latch in old.get_latches() {
        mapping[latch.output.index()] = Value::from(rebuilt.add_variable());
    }
    for gate in old.get_gates() {
        let var = rebuilt.add_variable();
        rebuilt.add_gate(
            gate.gate_type,
            var.xor(!gate.output.sign()),
            remap(gate.a, &mapping),
            remap(gate.b, &mapping),
        );
        mapping[gate.output.index()] = Value::from(var);
        *rebuilt.get_variable_label_mut(var.index()) =
            old.get_variable_label(gate.output.index()).clone();
    }
    for latch in old.get_latches() {
        let var = mapping[latch.output.index()].unwrap_variable();
        rebuilt.add_latch(
            remap(latch.input, &mapping),
            var.xor(!latch.output.sign()),
            latch.reset_value,
        );
        *rebuilt.get_variable_label_mut(var.index()) =
            old.get_variable_label(latch.output.index()).clone();
    }
    for (value, label) in old.get_outputs() {
        let index = rebuilt.add_output(remap(*value, &mapping));
        *rebuilt.get_output_label_mut(index) = label.clone();
    }
    for (value, label) in old.get_asserts() {
        let index = rebuilt.add_assert(remap(*value, &mapping));
        *rebuilt.get_assert_label_mut(index) = label.clone();
    }
    for (value, label) in old.get_assumes() {
        let index = rebuilt.add_assume(remap(*value, &mapping));
        *rebuilt.get_assume_label_mut(index) = label.clone();
    }
    for (value, label) in old.get_covers() {
        let index = rebuilt.add_cover(remap(*value, &mapping));
        *rebuilt.get_cover_label_mut(index) = label.clone();
    }
    for signal in (&mut model.inputs)
        .into_iter()
        .chain(&mut model.registers)
        .chain(&mut model.outputs)
    {
        for bit in &mut signal.bits {
            bit.value = remap(bit.value, &mapping);
        }
    }
    for property in (&mut model.assertions)
        .into_iter()
        .chain(&mut model.assumptions)
        .chain(&mut model.covers)
    {
        property.value = remap(property.value, &mapping);
    }
    model.inputs.retain(|signal| signal.name != reset.name);
    model.fsm = rebuilt;
    model.fsm.verify(VerifyOrdering::Verify);
    Ok(())
}
