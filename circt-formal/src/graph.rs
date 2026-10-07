use std::collections::{BTreeMap, BTreeSet};

use parser_circt::lexer::position::LexerPosition;

use parser_circt::ast::{
    ClockEdge, CombOperator, ConstantValue, File, HwModule, Item, Name, Operation, OperationKind,
    PortDirection, Signedness, Type, Value, Visibility,
};

use crate::{convert::ConversionOptions, error::ConvertError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Ty {
    Integer(usize, Signedness),
    Array(usize, Box<Ty>),
    Struct(Vec<(String, Ty)>),
    Union(Vec<(String, Ty, usize)>),
    Clock,
}

impl Ty {
    pub fn parse(ty: &Type) -> Result<Self, ConvertError> {
        let result = match ty {
            Type::Integer { width, signedness } if *width > 0 => {
                Self::Integer(*width as usize, *signedness)
            }
            Type::Array { size, element } if *size > 0 => Self::Array(
                usize::try_from(*size).map_err(|_| ConvertError::message("array size overflow"))?,
                Box::new(Self::parse(element)?),
            ),
            Type::Struct(fields) if !fields.is_empty() => Self::Struct(
                fields
                    .into_iter()
                    .map(|field| Ok((name(&field.name)?, Self::parse(&field.ty)?)))
                    .collect::<Result<_, ConvertError>>()?,
            ),
            Type::Union(fields) if !fields.is_empty() => {
                let mut names = BTreeSet::new();
                let mut members = Vec::new();
                for field in fields {
                    let name = name(&field.name)?;
                    if !names.insert(name.clone()) {
                        return Err(ConvertError::message("duplicate union member name"));
                    }
                    let offset = usize::try_from(field.offset.unwrap_or(0))
                        .map_err(|_| ConvertError::message("union field offset overflow"))?;
                    members.push((name, Self::parse(&field.ty)?, offset));
                }
                Self::Union(members)
            }
            Type::Clock => Self::Clock,
            _ => {
                return Err(ConvertError::message(
                    "unsupported type: zero-width data, empty aggregates, aliases and inouts are not supported",
                ));
            }
        };
        if result != Self::Clock {
            result.width()?;
        }
        Ok(result)
    }

    pub fn width(&self) -> Result<usize, ConvertError> {
        match self {
            Self::Integer(width, _) => Ok(*width),
            Self::Array(size, element) => size
                .checked_mul(element.width()?)
                .ok_or_else(|| ConvertError::message("array width overflow")),
            Self::Struct(fields) => fields.into_iter().try_fold(0usize, |width, (_, field)| {
                width
                    .checked_add(field.width()?)
                    .ok_or_else(|| ConvertError::message("struct width overflow"))
            }),
            Self::Union(fields) => {
                fields
                    .into_iter()
                    .try_fold(0usize, |width, (_, field, offset)| {
                        let end = offset
                            .checked_add(field.width()?)
                            .ok_or_else(|| ConvertError::message("union width overflow"))?;
                        Ok(width.max(end))
                    })
            }
            Self::Clock => Err(ConvertError::message("clock used as data")),
        }
    }

    pub fn is_bit(&self) -> bool {
        *self == bit_type()
    }

    pub fn union_field(&self, field: &str) -> Result<(&Ty, usize), ConvertError> {
        let Self::Union(fields) = self else {
            return Err(ConvertError::message("union_extract requires a union type"));
        };
        fields
            .into_iter()
            .find(|(name, _, _)| name == field)
            .map(|(_, ty, offset)| (ty, *offset))
            .ok_or_else(|| ConvertError::message(format!("unknown union member `{field}`")))
    }
}

pub(crate) fn bit_type() -> Ty {
    Ty::Integer(1, Signedness::Signless)
}

/// Decode MLIR quoted strings/symbols. SSA spellings are never display-name keys.
pub(crate) fn decode(bytes: &[u8]) -> Result<String, ConvertError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| ConvertError::message("invalid UTF-8 name"))?;
    let text = text
        .strip_prefix('%')
        .or_else(|| text.strip_prefix('@'))
        .unwrap_or(text);
    if !text.starts_with('"') {
        return Ok(text.to_owned());
    }
    let body = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(|| ConvertError::message("malformed quoted name"))?;
    let mut decoded = Vec::new();
    let mut chars = body.bytes();
    while let Some(byte) = chars.next() {
        if byte != b'\\' {
            decoded.push(byte);
            continue;
        }
        let first = chars
            .next()
            .ok_or_else(|| ConvertError::message("incomplete string escape"))?;
        match first {
            b'n' => decoded.push(b'\n'),
            b't' => decoded.push(b'\t'),
            b'"' => decoded.push(b'"'),
            b'\\' => decoded.push(b'\\'),
            _ => {
                let second = chars
                    .next()
                    .ok_or_else(|| ConvertError::message("incomplete hexadecimal escape"))?;
                let digit = |c: u8| {
                    (c as char)
                        .to_digit(16)
                        .map(|x| x as u8)
                        .ok_or_else(|| ConvertError::message("invalid hexadecimal escape"))
                };
                decoded.push(digit(first)? * 16 + digit(second)?);
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| ConvertError::message("name escape is not UTF-8"))
}

fn name(value: &Name) -> Result<String, ConvertError> {
    decode(&value.spelling)
}
fn key(value: &Value) -> Result<(String, usize), ConvertError> {
    Ok((name(&value.name)?, value.result_index.unwrap_or(0) as usize))
}

#[derive(Clone)]
pub(crate) enum Driver {
    Pending,
    Input,
    Alias(usize),
    Op(Box<Operation>, Vec<usize>),
}

#[derive(Clone)]
pub(crate) struct Node {
    pub ty: Ty,
    pub name: String,
    pub path: String,
    pub position: LexerPosition,
    pub location: Option<String>,
    pub driver: Driver,
}

impl Node {
    pub fn error(&self, message: impl Into<String>) -> ConvertError {
        match &self.driver {
            Driver::Op(op, _) => ConvertError::at(op, &self.path, message),
            _ => ConvertError {
                message: format!("{} ({})", message.into(), self.name),
                position: Some(self.position),
                location: self.location.clone(),
                instance: self.path.clone(),
            },
        }
    }
    pub fn register(&self) -> bool {
        match &self.driver {
            Driver::Op(op, _) => match op.kind {
                OperationKind::FirReg { .. } | OperationKind::CompReg { .. } => true,
                _ => false,
            },
            _ => false,
        }
    }
}

pub(crate) struct Property {
    pub op: Operation,
    pub args: Vec<usize>,
    pub path: String,
}

pub(crate) struct Graph {
    pub nodes: Vec<Node>,
    pub inputs: Vec<usize>,
    pub outputs: Vec<(String, usize)>,
    pub properties: Vec<Property>,
    pub top: String,
    pub clock: usize,
}

struct Entry<'a> {
    module: &'a HwModule,
    scope: usize,
    qualified: String,
}
struct Catalog<'a> {
    entries: Vec<Entry<'a>>,
    symbols: BTreeMap<(usize, String), usize>,
    parents: Vec<Option<usize>>,
}

impl<'a> Catalog<'a> {
    fn collect(
        &mut self,
        items: &'a [Item],
        scope: usize,
        prefix: &str,
    ) -> Result<(), ConvertError> {
        for item in items {
            match item {
                Item::Module(module) => {
                    let child = self.parents.len();
                    self.parents.push(Some(scope));
                    let prefix = if let Some(module_name) = &module.name {
                        qualify(prefix, &name(module_name)?)
                    } else {
                        prefix.to_owned()
                    };
                    self.collect(&module.items, child, &prefix)?;
                }
                Item::HwModule(module) => {
                    let local = name(&module.name)?;
                    let index = self.entries.len();
                    if self.symbols.insert((scope, local.clone()), index).is_some() {
                        return Err(ConvertError::message(format!("duplicate module {local}")));
                    }
                    self.entries.push(Entry {
                        module,
                        scope,
                        qualified: qualify(prefix, &local),
                    });
                }
            }
        }
        Ok(())
    }

    fn resolve(&self, mut scope: usize, symbol: &Name) -> Result<usize, ConvertError> {
        let local = name(symbol)?;
        loop {
            if let Some(index) = self.symbols.get(&(scope, local.clone())) {
                return Ok(*index);
            }
            match self.parents[scope] {
                Some(parent) => scope = parent,
                None => return Err(ConvertError::message(format!("unknown module {local}"))),
            }
        }
    }

    fn top(&self, requested: &Option<String>) -> Result<usize, ConvertError> {
        let mut referenced = BTreeSet::new();
        for entry in &self.entries {
            if let Some(body) = &entry.module.body {
                for op in body {
                    if let OperationKind::Instance { module, .. } = &op.kind
                        && let Ok(index) = self.resolve(entry.scope, module)
                    {
                        referenced.insert(index);
                    }
                }
            }
        }
        let candidates = (&self.entries)
            .into_iter()
            .enumerate()
            .filter(|(index, entry)| {
                if let Some(requested) = requested {
                    entry.qualified == *requested
                        || name(&entry.module.name).ok().as_ref() == Some(requested)
                } else {
                    entry.module.visibility == Visibility::Public
                        && entry.module.body.is_some()
                        && !referenced.contains(index)
                }
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(ConvertError::message(format!(
                "expected one top module, found {}; available: {}",
                candidates.len(),
                (&self.entries)
                    .into_iter()
                    .map(|entry| entry.qualified.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        Ok(candidates[0])
    }
}

fn qualify(path: &str, local: &str) -> String {
    if local.is_empty() {
        path.to_owned()
    } else if path.is_empty() {
        local.to_owned()
    } else {
        format!("{path}::{local}")
    }
}

impl Graph {
    pub fn build(file: &File, options: &ConversionOptions) -> Result<Self, ConvertError> {
        let mut catalog = Catalog {
            entries: Vec::new(),
            symbols: BTreeMap::new(),
            parents: vec![None],
        };
        catalog.collect(&file.items, 0, "")?;
        let top = catalog.top(&options.top)?;
        let mut graph = Self {
            nodes: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            properties: Vec::new(),
            top: catalog.entries[top].qualified.clone(),
            clock: usize::MAX,
        };
        let top_module = catalog.entries[top].module;
        let mut bindings = Vec::new();
        for port in &top_module.ports {
            if port.direction == PortDirection::InOut {
                return Err(ConvertError::message("inout ports are unsupported"));
            }
            if port.direction != PortDirection::Input {
                continue;
            }
            let port_name = name(&port.name)?;
            let id = graph.nodes.len();
            graph.nodes.push(Node {
                ty: Ty::parse(&port.ty).map_err(|mut error| {
                    error.position = Some(port.name.position);
                    error.instance = graph.top.clone();
                    error
                })?,
                name: port_name.clone(),
                path: graph.top.clone(),
                position: port.name.position,
                location: port
                    .location
                    .as_ref()
                    .map(|loc| String::from_utf8_lossy(&loc.spelling).into_owned()),
                driver: Driver::Input,
            });
            graph.inputs.push(id);
            bindings.push(id);
            if port_name == options.clock.name {
                graph.clock = id;
            }
        }
        if graph.clock == usize::MAX {
            return Err(ConvertError::message(format!(
                "unknown clock input {}",
                options.clock.name
            )));
        }
        if graph.nodes[graph.clock].ty != Ty::Clock && !graph.nodes[graph.clock].ty.is_bit() {
            return Err(ConvertError::message(
                "clock input must be i1 or !seq.clock",
            ));
        }
        if let Some(reset) = &options.reset {
            if reset.name == options.clock.name {
                return Err(
                    graph.nodes[graph.clock].error("clock and reset must name different signals")
                );
            }
            let input = (&graph.inputs)
                .into_iter()
                .copied()
                .find(|&id| graph.nodes[id].name == reset.name)
                .ok_or_else(|| {
                    ConvertError::message(format!("unknown reset input {}", reset.name))
                })?;
            if !graph.nodes[input].ty.is_bit() {
                return Err(graph.nodes[input].error("reset input must be i1"));
            }
        }
        let outputs = graph
            .expand(&catalog, top, &bindings, "", &mut Vec::new())
            .map_err(|mut error| {
                if error.position.is_none() {
                    error.position = Some(top_module.position);
                }
                if error.instance.is_empty() {
                    error.instance = graph.top.clone();
                }
                error
            })?;
        for (port, id) in (&top_module.ports)
            .into_iter()
            .filter(|port| port.direction == PortDirection::Output)
            .zip(outputs)
        {
            graph.outputs.push((name(&port.name)?, id));
        }
        graph.validate(options)?;
        Ok(graph)
    }

    fn expand(
        &mut self,
        catalog: &Catalog<'_>,
        index: usize,
        bindings: &[usize],
        path: &str,
        stack: &mut Vec<usize>,
    ) -> Result<Vec<usize>, ConvertError> {
        if stack.contains(&index) {
            return Err(ConvertError::message(format!(
                "recursive hierarchy in {path}"
            )));
        }
        stack.push(index);
        let entry = &catalog.entries[index];
        let body = entry.module.body.as_ref().ok_or_else(|| {
            ConvertError::message(format!(
                "external module {} instantiated in {path}",
                entry.qualified
            ))
        })?;
        let mut symbols = BTreeMap::new();
        let ports = (&entry.module.ports)
            .into_iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        if ports.len() != bindings.len() {
            return Err(ConvertError::message("instance input count mismatch"));
        }
        for (port, &id) in ports.into_iter().zip(bindings) {
            if Ty::parse(&port.ty)? != self.nodes[id].ty {
                return Err(ConvertError::message(format!(
                    "instance port type mismatch in {path}"
                )));
            }
            if symbols.insert((name(&port.name)?, 0), id).is_some() {
                return Err(ConvertError::message("duplicate input SSA name"));
            }
        }
        let mut port_names = BTreeSet::new();
        for port in &entry.module.ports {
            let direction = match port.direction {
                PortDirection::Input => 0,
                PortDirection::Output => 1,
                PortDirection::InOut => 2,
            };
            if !port_names.insert((direction, name(&port.name)?)) {
                return Err(ConvertError {
                    message: "duplicate port name".to_owned(),
                    position: Some(port.name.position),
                    location: None,
                    instance: qualify(&self.top, path),
                });
            }
            Ty::parse(&port.ty)?;
            if port.direction == PortDirection::InOut {
                return Err(ConvertError::message("inout ports are unsupported"));
            }
        }
        let mut results = Vec::new();
        for op in body {
            let types = result_types(&op.kind)
                .map_err(|err| ConvertError::at(op, &qualify(&self.top, path), err.message))?;
            let count = (&op.results)
                .into_iter()
                .map(|binding| binding.count as usize)
                .sum::<usize>();
            if count != types.len() {
                return Err(ConvertError::at(
                    op,
                    &qualify(&self.top, path),
                    "result count mismatch",
                ));
            }
            let mut op_results = Vec::new();
            let mut types = types.into_iter();
            for binding in &op.results {
                for result in 0..binding.count as usize {
                    let local = name(&binding.name)?;
                    let id = self.nodes.len();
                    if symbols.insert((local.clone(), result), id).is_some() {
                        return Err(ConvertError::at(
                            op,
                            &qualify(&self.top, path),
                            "duplicate SSA definition",
                        ));
                    }
                    let display = if binding.count == 1 {
                        local
                    } else {
                        format!("{local}#{result}")
                    };
                    self.nodes.push(Node {
                        ty: types.next().unwrap(),
                        name: qualify(path, &display),
                        path: qualify(&self.top, path),
                        position: op.position,
                        location: op
                            .location
                            .as_ref()
                            .map(|loc| String::from_utf8_lossy(&loc.spelling).into_owned()),
                        driver: Driver::Pending,
                    });
                    op_results.push(id);
                }
            }
            results.push(op_results);
        }
        let lookup = |value: &Value| -> Result<usize, ConvertError> {
            let key = key(value)?;
            symbols.get(&key).copied().ok_or_else(|| {
                ConvertError::message(format!("unresolved SSA value {}#{}", key.0, key.1))
            })
        };
        let mut outputs = None;
        let mut instances = BTreeSet::new();
        for (ordinal, op) in body.into_iter().enumerate() {
            let args = operands(&op.kind)
                .into_iter()
                .map(lookup)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| ConvertError::at(op, &qualify(&self.top, path), err.message))?;
            match &op.kind {
                OperationKind::Instance {
                    name: instance_name,
                    module,
                    inputs,
                    outputs: declared,
                } => {
                    let instance_name = decode(&instance_name.spelling)?;
                    if !instances.insert(instance_name.clone()) {
                        return Err(ConvertError::at(
                            op,
                            &qualify(&self.top, path),
                            "duplicate instance name",
                        ));
                    }
                    let child = catalog.resolve(entry.scope, module).map_err(|err| {
                        ConvertError::at(op, &qualify(&self.top, path), err.message)
                    })?;
                    let child_ports = &catalog.entries[child].module.ports;
                    let mut child_bindings = Vec::new();
                    let mut input_names = BTreeSet::new();
                    for input in inputs {
                        if !input_names.insert(name(&input.name)?) {
                            return Err(ConvertError::at(
                                op,
                                &qualify(&self.top, path),
                                "duplicate instance input",
                            ));
                        }
                    }
                    for port in child_ports
                        .into_iter()
                        .filter(|port| port.direction == PortDirection::Input)
                    {
                        let wanted = name(&port.name)?;
                        let matches = inputs
                            .into_iter()
                            .enumerate()
                            .filter(|(_, input)| name(&input.name).ok().as_ref() == Some(&wanted))
                            .collect::<Vec<_>>();
                        if matches.len() != 1 {
                            return Err(ConvertError::at(
                                op,
                                &qualify(&self.top, path),
                                format!("missing instance input {wanted}"),
                            ));
                        }
                        let (input_index, input) = matches[0];
                        if Ty::parse(&input.ty)? != Ty::parse(&port.ty)?
                            || self.nodes[args[input_index]].ty != Ty::parse(&port.ty)?
                        {
                            return Err(ConvertError::at(
                                op,
                                &qualify(&self.top, path),
                                "instance input type mismatch",
                            ));
                        }
                        child_bindings.push(args[input_index]);
                    }
                    if child_bindings.len() != inputs.len() {
                        return Err(ConvertError::at(
                            op,
                            &qualify(&self.top, path),
                            "extra instance input",
                        ));
                    }
                    let expected = child_ports
                        .into_iter()
                        .filter(|port| port.direction == PortDirection::Output)
                        .collect::<Vec<_>>();
                    if expected.len() != declared.len() {
                        return Err(ConvertError::at(
                            op,
                            &qualify(&self.top, path),
                            "instance output count mismatch",
                        ));
                    }
                    for (port, field) in expected.into_iter().zip(declared) {
                        if name(&port.name)? != name(&field.name)?
                            || Ty::parse(&port.ty)? != Ty::parse(&field.ty)?
                        {
                            return Err(ConvertError::at(
                                op,
                                &qualify(&self.top, path),
                                "instance output signature mismatch",
                            ));
                        }
                    }
                    let child_outputs = self
                        .expand(
                            catalog,
                            child,
                            &child_bindings,
                            &qualify(path, &instance_name),
                            stack,
                        )
                        .map_err(|mut err| {
                            if err.position.is_none() {
                                err.position = Some(op.position);
                                err.location = op
                                    .location
                                    .as_ref()
                                    .map(|loc| String::from_utf8_lossy(&loc.spelling).into_owned());
                            }
                            err
                        })?;
                    for (&result, output) in (&results[ordinal]).into_iter().zip(child_outputs) {
                        self.nodes[result].driver = Driver::Alias(output);
                    }
                }
                OperationKind::Output { types, .. } => {
                    if outputs.is_some() || ordinal + 1 != body.len() {
                        return Err(ConvertError::at(
                            op,
                            path,
                            "hw.output must be the unique final terminator",
                        ));
                    }
                    let ports = (&entry.module.ports)
                        .into_iter()
                        .filter(|port| port.direction == PortDirection::Output)
                        .collect::<Vec<_>>();
                    if ports.len() != args.len() || types.len() != args.len() {
                        return Err(ConvertError::at(
                            op,
                            &qualify(&self.top, path),
                            "output count mismatch",
                        ));
                    }
                    for ((port, ty), &id) in ports.into_iter().zip(types).zip(&args) {
                        if Ty::parse(&port.ty)? != Ty::parse(ty)?
                            || self.nodes[id].ty != Ty::parse(ty)?
                        {
                            return Err(ConvertError::at(
                                op,
                                &qualify(&self.top, path),
                                "output type mismatch",
                            ));
                        }
                    }
                    outputs = Some(args);
                }
                OperationKind::ClockedProperty { .. } => self.properties.push(Property {
                    op: op.clone(),
                    args,
                    path: qualify(&self.top, path),
                }),
                _ => {
                    self.nodes[results[ordinal][0]].driver = Driver::Op(Box::new(op.clone()), args)
                }
            }
        }
        stack.pop();
        outputs.ok_or_else(|| {
            ConvertError::message(format!("missing hw.output in {}", qualify(&self.top, path)))
        })
    }

    fn validate(&self, options: &ConversionOptions) -> Result<(), ConvertError> {
        let mut visits = vec![0u8; self.nodes.len()];
        for id in 0..self.nodes.len() {
            self.visit(id, &mut visits)?;
        }
        for node in &self.nodes {
            if let Driver::Op(op, args) = &node.driver {
                self.validate_op(node, op, args).map_err(|error| {
                    if error.position.is_none() {
                        ConvertError::at(op, &node.path, error.message)
                    } else {
                        error
                    }
                })?;
                if node.register() {
                    self.check_clock(args[1], ClockEdge::Positive, options, node)?;
                }
            }
        }
        for property in &self.properties {
            let OperationKind::ClockedProperty { ty, edge, .. } = &property.op.kind else {
                unreachable!()
            };
            if Ty::parse(ty)
                .map_err(|error| ConvertError::at(&property.op, &property.path, error.message))?
                != bit_type()
                || !self.nodes[property.args[0]].ty.is_bit()
            {
                return Err(ConvertError::at(
                    &property.op,
                    &property.path,
                    "property must have type i1",
                ));
            }
            if property.args.len() == 3 && !self.nodes[property.args[1]].ty.is_bit() {
                return Err(ConvertError::at(
                    &property.op,
                    &property.path,
                    "property enable must be i1",
                ));
            }
            if !self.nodes[*property.args.last().unwrap()].ty.is_bit() {
                return Err(ConvertError::at(
                    &property.op,
                    &property.path,
                    "clocked property clock must be i1",
                ));
            }
            self.check_clock(
                *property.args.last().unwrap(),
                *edge,
                options,
                &Node {
                    ty: bit_type(),
                    name: String::new(),
                    path: property.path.clone(),
                    position: property.op.position,
                    location: None,
                    driver: Driver::Op(Box::new(property.op.clone()), property.args.clone()),
                },
            )?;
        }
        // Reject the selected clock anywhere in the ordinary data graph. Other
        // clock inputs are permitted only if they are unused i1 data inputs.
        for (id, node) in (&self.nodes).into_iter().enumerate() {
            if let Driver::Op(op, args) = &node.driver {
                let data_args: &[usize] = match &op.kind {
                    OperationKind::ToClock { .. } => &[],
                    OperationKind::FirReg { .. } | OperationKind::CompReg { .. } => &args[..1],
                    _ => args,
                };
                // Clock inversion is wiring, not ordinary data, if every use is
                // clock wiring. Data consumers are checked recursively below.
                if !self.is_clock_wiring(id) {
                    for &arg in data_args {
                        self.data(arg, &mut BTreeSet::new())?;
                    }
                }
                if node.register() {
                    for &arg in &args[2..] {
                        self.data(arg, &mut BTreeSet::new())?;
                    }
                }
            }
        }
        for (_, id) in &self.outputs {
            self.data(*id, &mut BTreeSet::new())?;
        }
        for property in &self.properties {
            for &id in &property.args[..property.args.len() - 1] {
                self.data(id, &mut BTreeSet::new())?;
            }
        }
        Ok(())
    }

    fn visit(&self, id: usize, visits: &mut [u8]) -> Result<(), ConvertError> {
        if visits[id] == 2 || self.nodes[id].register() {
            return Ok(());
        }
        if visits[id] == 1 {
            return Err(self.nodes[id].error("combinational cycle"));
        }
        visits[id] = 1;
        match &self.nodes[id].driver {
            Driver::Alias(other) => self.visit(*other, visits)?,
            Driver::Op(_, args) => {
                for &arg in args {
                    self.visit(arg, visits)?;
                }
            }
            Driver::Pending => return Err(self.nodes[id].error("unresolved graph node")),
            Driver::Input => {}
        }
        visits[id] = 2;
        Ok(())
    }

    fn validate_op(&self, node: &Node, op: &Operation, args: &[usize]) -> Result<(), ConvertError> {
        let fail = |message| node.error(message);
        let types = args
            .into_iter()
            .map(|&id| &self.nodes[id].ty)
            .collect::<Vec<_>>();
        match &op.kind {
            OperationKind::Constant {
                value: ConstantValue::Boolean(_),
                ..
            } if !node.ty.is_bit() => return Err(fail("boolean constant must be i1")),
            OperationKind::Constant { .. } if !matches_integer(&node.ty) => {
                return Err(fail("constant must have integer type"));
            }
            OperationKind::Comb { operator, .. } => {
                let variadic = [
                    CombOperator::Add,
                    CombOperator::Mul,
                    CombOperator::And,
                    CombOperator::Or,
                    CombOperator::Xor,
                ]
                .contains(operator);
                if !matches_integer(&node.ty)
                    || types.is_empty()
                    || (!variadic && types.len() != 2)
                    || types.into_iter().any(|ty| *ty != node.ty)
                {
                    return Err(fail("comb operand arity/type mismatch"));
                }
            }
            OperationKind::Compare { ty, .. } => {
                let expected = Ty::parse(ty)?;
                if !matches_integer(&expected)
                    || types.len() != 2
                    || types.into_iter().any(|ty| *ty != expected)
                {
                    return Err(fail("comparison operand type mismatch"));
                }
            }
            OperationKind::Mux { .. } => {
                if !types[0].is_bit() || *types[1] != node.ty || *types[2] != node.ty {
                    return Err(fail("mux operand type mismatch"));
                }
            }
            OperationKind::Concat {
                types: declared, ..
            } => {
                if types.is_empty() || types.len() != declared.len() {
                    return Err(fail("concat operand count mismatch"));
                }
                for (ty, declared) in types.into_iter().zip(declared) {
                    if !matches_integer(ty) || *ty != Ty::parse(declared)? {
                        return Err(fail("concat operand type mismatch"));
                    }
                }
            }
            OperationKind::UnionExtract {
                union_type, field, ..
            } => {
                let expected = Ty::parse(union_type)?;
                let (member, _) = expected.union_field(&decode(&field.spelling)?)?;
                if *types[0] != expected || node.ty != *member {
                    return Err(fail("union_extract operand/result type mismatch"));
                }
            }
            OperationKind::Bitcast { input_type, .. } => {
                if *types[0] != Ty::parse(input_type)? {
                    return Err(fail("bitcast operand type mismatch"));
                }
                if types[0].width()? != node.ty.width()? {
                    return Err(fail("bitcast input/result width mismatch"));
                }
            }
            OperationKind::Replicate { input_type, .. } => {
                if !matches_integer(&node.ty)
                    || !matches_integer(types[0])
                    || *types[0] != Ty::parse(input_type)?
                {
                    return Err(fail("replicate operand/result type mismatch"));
                }
                let input_width = types[0].width()?;
                let result_width = node.ty.width()?;
                if result_width < input_width || !result_width.is_multiple_of(input_width) {
                    return Err(fail(
                        "replicate result width must be a positive multiple of input width",
                    ));
                }
            }
            OperationKind::Extract {
                input_type, offset, ..
            } => {
                if !matches_integer(&node.ty)
                    || !matches_integer(types[0])
                    || *types[0] != Ty::parse(input_type)?
                    || usize::try_from(*offset)
                        .ok()
                        .and_then(|o| o.checked_add(node.ty.width().ok()?))
                        .is_none_or(|end| end > types[0].width().unwrap())
                {
                    return Err(fail("extract type/range mismatch"));
                }
            }
            OperationKind::ArrayGet {
                array_type,
                index_type,
                ..
            }
            | OperationKind::ArrayInject {
                array_type,
                index_type,
                ..
            } => {
                let expected = Ty::parse(array_type)?;
                let Ty::Array(size, element) = &expected else {
                    return Err(fail("array operation requires array type"));
                };
                let expected_index_width = if *size == 1 {
                    1
                } else {
                    usize::BITS as usize - (size - 1).leading_zeros() as usize
                };
                if *types[0] != expected
                    || *types[1] != Ty::parse(index_type)?
                    || !matches_integer(types[1])
                    || types[1].width()? != expected_index_width
                {
                    return Err(fail("array operand/index type mismatch"));
                }
                if types.len() == 3 && *types[2] != **element {
                    return Err(fail("array injected element type mismatch"));
                }
            }
            OperationKind::ToClock { .. } => {
                if !types[0].is_bit() {
                    return Err(fail("seq.to_clock requires i1"));
                }
            }
            OperationKind::FirReg { .. } | OperationKind::CompReg { .. } => {
                if *types[0] != node.ty || *types[1] != Ty::Clock {
                    return Err(fail("register data/clock type mismatch"));
                }
                if types.len() == 4 && (!types[2].is_bit() || *types[3] != node.ty) {
                    return Err(fail("register reset type mismatch"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn clock_source(&self, id: usize) -> Result<(usize, bool), ConvertError> {
        let node = &self.nodes[id];
        match &node.driver {
            Driver::Input => Ok((id, false)),
            Driver::Alias(other) => self.clock_source(*other),
            Driver::Op(op, args) => match &op.kind {
                OperationKind::ToClock { .. } => self.clock_source(args[0]),
                OperationKind::Comb {
                    operator: CombOperator::Xor,
                    ..
                } if node.ty.is_bit() => {
                    let mut source = None;
                    let mut inverted = false;
                    for &arg in args {
                        if let Some(value) = self.constant_bit(arg) {
                            inverted ^= value;
                        } else if source.is_none() {
                            source = Some(self.clock_source(arg)?);
                        } else {
                            return Err(node.error("derived/gated clock is unsupported"));
                        }
                    }
                    let (root, flip) =
                        source.ok_or_else(|| node.error("constant clock is unsupported"))?;
                    Ok((root, flip ^ inverted))
                }
                _ => Err(node.error("derived/gated clock is unsupported")),
            },
            Driver::Pending => Err(node.error("unresolved clock")),
        }
    }

    fn constant_bit(&self, id: usize) -> Option<bool> {
        match &self.nodes[id].driver {
            Driver::Alias(other) => self.constant_bit(*other),
            Driver::Op(op, _) if self.nodes[id].ty.is_bit() => match &op.kind {
                OperationKind::Constant {
                    value: ConstantValue::Boolean(value),
                    ..
                } => Some(*value),
                OperationKind::Constant {
                    value: ConstantValue::Integer(value),
                    ..
                } => {
                    let text = std::str::from_utf8(&value.spelling).ok()?;
                    let unsigned = text.trim_start_matches(['-', '+']);
                    let radix = if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
                        16
                    } else {
                        10
                    };
                    Some(unsigned.chars().last()?.to_digit(radix)? & 1 != 0)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn check_clock(
        &self,
        id: usize,
        edge: ClockEdge,
        options: &ConversionOptions,
        node: &Node,
    ) -> Result<(), ConvertError> {
        let (root, inverted) = self.clock_source(id)?;
        let negative = (edge == ClockEdge::Negative) ^ inverted;
        if root != self.clock || negative != (options.clock.edge == ClockEdge::Negative) {
            return Err(node.error(format!(
                "clock domain does not match {:?} edge of {}",
                options.clock.edge, options.clock.name
            )));
        }
        Ok(())
    }

    pub fn is_clock_wiring(&self, id: usize) -> bool {
        self.clock_source(id)
            .is_ok_and(|(root, _)| root == self.clock)
    }

    fn data(&self, id: usize, seen: &mut BTreeSet<usize>) -> Result<(), ConvertError> {
        if id == self.clock || self.nodes[id].ty == Ty::Clock {
            return Err(self.nodes[id].error("clock used as ordinary data"));
        }
        if !seen.insert(id) || self.nodes[id].register() {
            return Ok(());
        }
        match &self.nodes[id].driver {
            Driver::Alias(other) => self.data(*other, seen)?,
            Driver::Op(_, args) => {
                for &arg in args {
                    self.data(arg, seen)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn matches_integer(ty: &Ty) -> bool {
    match ty {
        Ty::Integer(_, _) => true,
        _ => false,
    }
}

fn result_types(kind: &OperationKind) -> Result<Vec<Ty>, ConvertError> {
    let ty = match kind {
        OperationKind::Constant { ty, .. }
        | OperationKind::Comb { ty, .. }
        | OperationKind::Mux { ty, .. }
        | OperationKind::FirReg { ty, .. }
        | OperationKind::CompReg { ty, .. } => Ty::parse(ty)?,
        OperationKind::Compare { .. } => bit_type(),
        OperationKind::ToClock { .. } => Ty::Clock,
        OperationKind::UnionExtract {
            union_type, field, ..
        } => Ty::parse(union_type)?
            .union_field(&decode(&field.spelling)?)?
            .0
            .clone(),
        OperationKind::Bitcast { result_type, .. }
        | OperationKind::Replicate { result_type, .. }
        | OperationKind::Extract { result_type, .. } => Ty::parse(result_type)?,
        OperationKind::ArrayInject { array_type, .. } => Ty::parse(array_type)?,
        OperationKind::ArrayGet { array_type, .. } => match Ty::parse(array_type)? {
            Ty::Array(_, element) => *element,
            _ => return Err(ConvertError::message("hw.array_get requires an array")),
        },
        OperationKind::Concat { types, .. } => {
            let width = types.into_iter().try_fold(0usize, |total, ty| {
                total
                    .checked_add(Ty::parse(ty)?.width()?)
                    .ok_or_else(|| ConvertError::message("concat width overflow"))
            })?;
            if width == 0 {
                return Err(ConvertError::message("zero-width concat"));
            }
            Ty::Integer(width, Signedness::Signless)
        }
        OperationKind::Instance { outputs, .. } => {
            return outputs
                .into_iter()
                .map(|field| Ty::parse(&field.ty))
                .collect();
        }
        OperationKind::Output { .. } | OperationKind::ClockedProperty { .. } => {
            return Ok(Vec::new());
        }
    };
    Ok(vec![ty])
}

fn operands(kind: &OperationKind) -> Vec<&Value> {
    match kind {
        OperationKind::Constant { .. } => Vec::new(),
        OperationKind::Output { values, .. } => values.into_iter().collect(),
        OperationKind::Instance { inputs, .. } => {
            inputs.into_iter().map(|input| &input.value).collect()
        }
        OperationKind::ArrayGet { array, index, .. } => vec![array, index],
        OperationKind::ArrayInject {
            array,
            index,
            value,
            ..
        } => vec![array, index, value],
        OperationKind::Comb { operands, .. } | OperationKind::Concat { operands, .. } => {
            operands.into_iter().collect()
        }
        OperationKind::Compare { lhs, rhs, .. } => vec![lhs, rhs],
        OperationKind::Mux {
            condition,
            true_value,
            false_value,
            ..
        } => vec![condition, true_value, false_value],
        OperationKind::UnionExtract { input, .. }
        | OperationKind::Bitcast { input, .. }
        | OperationKind::Replicate { input, .. }
        | OperationKind::Extract { input, .. }
        | OperationKind::ToClock { input } => vec![input],
        OperationKind::FirReg {
            input,
            clock,
            reset,
            ..
        }
        | OperationKind::CompReg {
            input,
            clock,
            reset,
            ..
        } => {
            let mut args = vec![input, clock];
            if let Some(reset) = reset {
                args.extend([&reset.signal, &reset.value]);
            }
            args
        }
        OperationKind::ClockedProperty {
            property,
            enable,
            clock,
            ..
        } => {
            let mut args = vec![property];
            if let Some(enable) = enable {
                args.push(enable);
            }
            args.push(clock);
            args
        }
    }
}
