mod error;
#[cfg(test)]
mod tests;
mod util;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs,
    path::{Component, Path, PathBuf},
};

use verilator_parser::ast::{
    AssignmentTarget, BinaryOperator, DataType, DataTypeKind, Design, Direction, Domain,
    Expression, ExpressionKind, Literal, PropertyKind, SignalDomain, Statement, StatementKind,
    UnaryOperator, VariableId, collect::CollectAccesses,
};

use crate::{
    error::GenerateError,
    util::{
        bits_to_public, public_to_bits, public_value_type, screaming_identifier, snake_identifier,
        type_identifier, unique_name, unique_names,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateOptions {
    pub crate_name: String,
    pub clock: Domain,
    pub reset: Option<Domain>,
}

pub fn generate_project(
    design: &Design,
    output_dir: impl AsRef<Path>,
    options: &GenerateOptions,
) -> Result<(), GenerateError> {
    let output_dir = output_dir.as_ref();
    validate_crate_name(&options.crate_name)?;
    let (clock, reset) = select_domains(design, &options.clock, options.reset.as_ref())?;
    let sources = Generator::new(design, clock, reset).generate()?;
    let absolute_output = absolute_normalized(output_dir)?;
    let runtime_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verilator-rust-runtime");
    let runtime_path = relative_path(&absolute_output, &runtime_dir)?;
    let manifest = format!(
        "[package]\nname = {:?}\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nverilator-rust-runtime = {{ path = {:?} }}\n\n[workspace]\n",
        options.crate_name,
        runtime_path.to_string_lossy(),
    );

    if output_dir.exists() {
        let mut entries = fs::read_dir(output_dir).map_err(|error| {
            GenerateError::message(format!(
                "could not inspect output directory {}: {error}",
                output_dir.display()
            ))
        })?;
        if entries.next().is_some() {
            return Err(GenerateError::message(format!(
                "output directory {} is not empty",
                output_dir.display()
            )));
        }
    }
    let source_dir = output_dir.join("src");
    fs::create_dir_all(&source_dir).map_err(|error| {
        GenerateError::message(format!(
            "could not create output directory {}: {error}",
            source_dir.display()
        ))
    })?;
    fs::write(output_dir.join("Cargo.toml"), manifest).map_err(|error| {
        GenerateError::message(format!("could not write generated Cargo.toml: {error}"))
    })?;
    for (file, source) in [
        ("lib.rs", sources.lib_rs),
        ("types.rs", sources.types_rs),
        ("input.rs", sources.input_rs),
        ("output.rs", sources.output_rs),
        ("state.rs", sources.state_rs),
    ] {
        fs::write(source_dir.join(file), source).map_err(|error| {
            GenerateError::message(format!("could not write generated src/{file}: {error}"))
        })?;
    }
    Ok(())
}

fn absolute_normalized(path: &Path) -> Result<PathBuf, GenerateError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                GenerateError::message(format!("could not read current directory: {error}"))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(GenerateError::message(format!(
                        "could not normalize path {}",
                        absolute.display()
                    )));
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    let mut existing = normalized.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            GenerateError::message(format!("could not resolve path {}", normalized.display()))
        })?;
        missing.push(name.to_os_string());
        if !existing.pop() {
            return Err(GenerateError::message(format!(
                "could not resolve path {}",
                normalized.display()
            )));
        }
    }
    let mut resolved = fs::canonicalize(&existing).map_err(|error| {
        GenerateError::message(format!(
            "could not resolve path {}: {error}",
            existing.display()
        ))
    })?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn relative_path(from: &Path, to: &Path) -> Result<PathBuf, GenerateError> {
    let from = absolute_normalized(from)?;
    let to = absolute_normalized(to)?;
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = (&from_components)
        .into_iter()
        .zip(&to_components)
        .take_while(|(from, to)| from == to)
        .count();
    if common == 0 {
        return Err(GenerateError::message(format!(
            "output {} and runtime {} do not share a path root",
            from.display(),
            to.display()
        )));
    }
    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    Ok(relative)
}

fn validate_crate_name(name: &str) -> Result<(), GenerateError> {
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
        || !name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
    {
        return Err(GenerateError::message(format!(
            "invalid Cargo package name {name:?}"
        )));
    }
    Ok(())
}

fn select_domains(
    design: &Design,
    clock: &Domain,
    reset: Option<&Domain>,
) -> Result<(SignalDomain, Option<SignalDomain>), GenerateError> {
    if reset.is_some_and(|reset| reset.name == clock.name) {
        return Err(GenerateError::message(
            "clock and reset must name different signals",
        ));
    }
    let clock_domain = (&design.sensitivity_domains)
        .into_iter()
        .find(|domain| domain.domain == *clock)
        .cloned()
        .ok_or_else(|| GenerateError::message(format!("clock {clock:?} was not found")))?;
    let reset_domain = reset
        .map(|reset| resolve_reset(design, reset))
        .transpose()?;
    for domain in &design.sensitivity_domains {
        if *domain == clock_domain || reset_domain.as_ref() == Some(domain) {
            continue;
        }
        return Err(GenerateError::message(format!(
            "encountered sensitivity domain {domain:?} but expected clock {clock_domain:?}"
        )));
    }
    Ok((clock_domain, reset_domain))
}

fn resolve_reset(design: &Design, reset: &Domain) -> Result<SignalDomain, GenerateError> {
    let matches = (&design.variables)
        .into_iter()
        .enumerate()
        .filter(|(_, variable)| variable.display_name() == reset.name)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(GenerateError::message(format!(
            "expected exactly one reset input named {}, found {}",
            reset.name,
            matches.len()
        )));
    }
    let (index, variable) = matches[0];
    if variable.direction != Direction::Input || design.data_type(variable.dtype).width != 1 {
        return Err(GenerateError::source(
            &variable.source,
            "reset must be a one-bit input",
        ));
    }
    Ok(SignalDomain {
        variable: VariableId(index),
        domain: reset.clone(),
    })
}

struct Generator<'a> {
    design: &'a Design,
    clock: SignalDomain,
    reset: Option<SignalDomain>,
    field_names: Vec<String>,
    type_names: Vec<Option<String>>,
    temporary: usize,
}

#[derive(Debug)]
struct SignalField {
    name: String,
    ids: Vec<VariableId>,
    array: bool,
}

struct GeneratedSources {
    lib_rs: String,
    types_rs: String,
    input_rs: String,
    output_rs: String,
    state_rs: String,
}

fn with_to_usize(value: String, to_usize: bool) -> String {
    if to_usize {
        format!("{value}.to_usize()")
    } else {
        value
    }
}

fn resize_read(value: &str, source_width: usize, target_width: usize, signed: bool) -> String {
    if source_width == target_width {
        value.to_string()
    } else {
        format!("({value}).resize::<{target_width}>({signed})")
    }
}

fn resize_owned(
    expression: String,
    source_width: usize,
    target_width: usize,
    signed: bool,
) -> String {
    if source_width == target_width {
        expression
    } else {
        format!("{expression}.resize::<{target_width}>({signed})")
    }
}

impl<'a> Generator<'a> {
    fn new(design: &'a Design, clock: SignalDomain, reset: Option<SignalDomain>) -> Self {
        let field_names = unique_names(
            (&design.variables)
                .into_iter()
                .map(|variable| snake_identifier(variable.display_name())),
        );
        let mut used_types = BTreeMap::new();
        for reserved in [
            "Bits",
            "Inputs",
            "Outputs",
            "State",
            "Assertions",
            "Assumptions",
            "Covers",
            "Evaluation",
            "Model",
        ] {
            used_types.insert(reserved.to_string(), 1);
        }
        let type_names = (&design.data_types)
            .into_iter()
            .map(|dtype| {
                let name = dtype
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or("RtlType");
                Some(unique_name(type_identifier(name), &mut used_types))
            })
            .collect();
        Self {
            design,
            clock,
            reset,
            field_names,
            type_names,
            temporary: 0,
        }
    }

    fn generate(mut self) -> Result<GeneratedSources, GenerateError> {
        let (combinational, fixed_point) = order_combinational(self.design);
        let input_ids = self.input_ids();
        let state_ids = self.state_ids();
        let output_ids = self.output_ids();
        let input_fields = self.signal_fields(&input_ids);
        let state_fields = self.signal_fields(&state_ids);
        let output_fields = self.signal_fields(&output_ids);
        let mut lib_rs = String::new();
        writeln!(
            lib_rs,
            "//! Standalone two-state simulator generated by verilator-rust.\n"
        )
        .unwrap();
        writeln!(
            lib_rs,
            "#![allow(dead_code, non_camel_case_types, unused_imports)]\n"
        )
        .unwrap();
        writeln!(lib_rs, "mod input;").unwrap();
        writeln!(lib_rs, "mod output;").unwrap();
        writeln!(lib_rs, "mod state;").unwrap();
        writeln!(lib_rs, "mod types;\n").unwrap();
        writeln!(lib_rs, "pub use input::Inputs;").unwrap();
        writeln!(lib_rs, "pub use output::Outputs;").unwrap();
        writeln!(lib_rs, "pub use state::State;").unwrap();
        writeln!(lib_rs, "pub use types::*;\n").unwrap();
        writeln!(
            lib_rs,
            "// Selected clock: {:?} {:?}",
            self.clock.domain.name, self.clock.domain.edge
        )
        .unwrap();
        if let Some(reset) = &self.reset {
            writeln!(
                lib_rs,
                "// Reset sensitivity: {:?} {:?}",
                reset.domain.name, reset.domain.edge
            )
            .unwrap();
        }
        writeln!(
            lib_rs,
            "use verilator_rust_runtime::{{Bits, array_offset, bool_and, bool_or, bool_xor}};\n"
        )
        .unwrap();

        let mut types_rs = String::from(
            "//! Generated RTL data types.\n\nuse verilator_rust_runtime::{Bits, array_offset};\n\n",
        );
        self.emit_data_types(&mut types_rs);
        let mut input_rs =
            String::from("//! Generated model inputs.\n\nuse verilator_rust_runtime::Bits;\n\n");
        self.emit_signal_struct("Inputs", &input_fields, &mut input_rs);
        let mut output_rs =
            String::from("//! Generated model outputs.\n\nuse verilator_rust_runtime::Bits;\n\n");
        self.emit_signal_struct("Outputs", &output_fields, &mut output_rs);
        let mut state_rs =
            String::from("//! Generated model state.\n\nuse verilator_rust_runtime::Bits;\n\n");
        self.emit_signal_struct("State", &state_fields, &mut state_rs);

        self.emit_property_struct("Assertions", PropertyKind::Assertion, &mut lib_rs);
        self.emit_property_struct("Assumptions", PropertyKind::Assumption, &mut lib_rs);
        self.emit_property_struct("Covers", PropertyKind::Cover, &mut lib_rs);
        writeln!(
            lib_rs,
            "#[derive(Clone, Debug, Default, PartialEq, Eq)]\npub struct Evaluation {{\n    pub outputs: Outputs,\n    pub assertions: Assertions,\n    pub assumptions: Assumptions,\n    pub covers: Covers,\n}}\n"
        )
        .unwrap();
        writeln!(
            lib_rs,
            "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\nstruct Env {{"
        )
        .unwrap();
        for (index, variable) in (&self.design.variables).into_iter().enumerate() {
            let width = self.design.data_type(variable.dtype).width;
            writeln!(lib_rs, "    v{index}: Bits<{width}>,").unwrap();
        }
        writeln!(lib_rs, "}}\n").unwrap();
        writeln!(
            lib_rs,
            "#[derive(Clone, Debug)]\npub struct Model {{\n    values: Env,\n    state: State,\n}}\n"
        )
        .unwrap();

        self.emit_apply_inputs(&input_fields, &mut lib_rs);
        self.emit_phase("run_initial", &self.design.initial, &mut lib_rs);
        if fixed_point {
            self.emit_phase("run_combinational_pass", &combinational, &mut lib_rs);
            writeln!(
                lib_rs,
                "
fn run_combinational(env: &mut Env) {{
    for _ in 0..={variable_count} {{
        let before = *env;
        run_combinational_pass(env);
        if *env == before {{ return; }}
    }}
    panic!(\"combinational logic did not reach a fixed point\");
}}
",
                variable_count = self.design.variables.len(),
            )
            .unwrap();
        } else {
            self.emit_phase("run_combinational", &combinational, &mut lib_rs);
        }
        self.emit_phase("run_pre_edge", &self.design.pre_edge, &mut lib_rs);
        self.emit_phase("run_sequential", &self.design.sequential, &mut lib_rs);
        self.emit_phase("run_post_edge", &self.design.post_edge, &mut lib_rs);
        self.emit_shadow_functions(&mut lib_rs);
        self.emit_make_state(&state_fields, &mut lib_rs);
        self.emit_evaluation(&output_fields, &mut lib_rs);
        writeln!(
            lib_rs,
            "impl Default for Model {{
    fn default() -> Self {{ Self::new() }}
}}

impl Model {{
    pub fn new() -> Self {{
        let mut values = Env::default();
        run_initial(&mut values);
        let state = make_state(&values);
        Self {{ values, state }}
    }}

    pub fn state(&self) -> &State {{ &self.state }}

    pub fn eval(&self, inputs: &Inputs) -> Evaluation {{
        let mut env = self.values;
        apply_inputs(&mut env, inputs);
        run_combinational(&mut env);
        make_evaluation(&env)
    }}

    pub fn tick(&mut self, inputs: &Inputs) -> Evaluation {{
        let mut env = self.values;
        apply_inputs(&mut env, inputs);
        run_combinational(&mut env);
        prepare_shadows(&mut env);
        run_pre_edge(&mut env);
        run_sequential(&mut env);
        run_post_edge(&mut env);
        commit_shadows(&mut env);
        self.values = env;
        self.state = make_state(&self.values);
        self.eval(inputs)
    }}
}}
"
        )
        .unwrap();
        Ok(GeneratedSources {
            lib_rs,
            types_rs,
            input_rs,
            output_rs,
            state_rs,
        })
    }

    fn input_ids(&self) -> Vec<VariableId> {
        (&self.design.variables)
            .into_iter()
            .enumerate()
            .filter(|(index, variable)| {
                variable.direction == Direction::Input && VariableId(*index) != self.clock.variable
            })
            .map(|(index, _)| VariableId(index))
            .collect()
    }

    fn output_ids(&self) -> Vec<VariableId> {
        (&self.design.variables)
            .into_iter()
            .enumerate()
            .filter(|(_, variable)| variable.direction == Direction::Output)
            .map(|(index, _)| VariableId(index))
            .collect()
    }

    fn state_ids(&self) -> Vec<VariableId> {
        let shadows = (&self.design.shadow_registers)
            .into_iter()
            .map(|(shadow, _)| *shadow)
            .collect::<BTreeSet<_>>();
        let mut ids = (&self.design.shadow_registers)
            .into_iter()
            .map(|(_, register)| *register)
            .collect::<BTreeSet<_>>();
        let mut writes = BTreeSet::new();
        for statement in (&self.design.initial)
            .into_iter()
            .chain(&self.design.pre_edge)
            .chain(&self.design.sequential)
            .chain(&self.design.post_edge)
        {
            statement.collect_writes(&mut writes);
        }
        ids.extend(writes.into_iter().filter(|id| !shadows.contains(id)));
        ids.into_iter().collect()
    }

    fn signal_fields(&self, ids: &[VariableId]) -> Vec<SignalField> {
        let mut candidates = BTreeMap::<String, BTreeMap<usize, VariableId>>::new();
        let mut invalid = BTreeSet::new();
        for id in ids {
            let field_name = &self.field_names[id.0];
            let Some((base, index)) = indexed_field_name(field_name) else {
                continue;
            };
            if candidates
                .entry(base.to_string())
                .or_default()
                .insert(index, *id)
                .is_some()
            {
                invalid.insert(base.to_string());
            }
        }

        let names = ids
            .into_iter()
            .map(|id| self.field_names[id.0].as_str())
            .collect::<BTreeSet<_>>();
        let arrays = candidates
            .into_iter()
            .filter_map(|(base, indexed)| {
                if invalid.contains(&base) || names.contains(base.as_str()) {
                    return None;
                }
                let elements = indexed.values().copied().collect::<Vec<_>>();
                let dense = indexed.keys().copied().eq(0..indexed.len());
                let width = elements
                    .first()
                    .map(|id| self.design.data_type(self.design.variable(*id).dtype).width)?;
                let same_type = (&elements).into_iter().all(|id| {
                    self.design.data_type(self.design.variable(*id).dtype).width == width
                });
                (dense && same_type).then_some((base, elements))
            })
            .collect::<BTreeMap<_, _>>();

        let mut fields = Vec::new();
        for id in ids {
            let field_name = &self.field_names[id.0];
            if let Some((base, index)) = indexed_field_name(field_name)
                && let Some(elements) = arrays.get(base)
            {
                if index == 0 {
                    fields.push(SignalField {
                        name: base.to_string(),
                        ids: elements.clone(),
                        array: true,
                    });
                }
                continue;
            }
            fields.push(SignalField {
                name: field_name.clone(),
                ids: vec![*id],
                array: false,
            });
        }
        let field_names = unique_names(
            (&fields)
                .into_iter()
                .map(|field| snake_identifier(&field.name)),
        );
        for (field, name) in fields.iter_mut().zip(field_names) {
            field.name = name;
        }
        fields
    }

    fn emit_signal_struct(&self, name: &str, fields: &[SignalField], source: &mut String) {
        writeln!(source, "#[derive(Clone, Debug, PartialEq, Eq)]").unwrap();
        writeln!(source, "pub struct {name} {{").unwrap();
        for field in fields {
            let variable = self.design.variable(field.ids[0]);
            let width = self.design.data_type(variable.dtype).width;
            if field.array {
                writeln!(
                    source,
                    "    /// RTL signals {:?} through {:?}.",
                    variable.display_name(),
                    self.design
                        .variable(*field.ids.last().unwrap())
                        .display_name()
                )
                .unwrap();
            } else {
                writeln!(source, "    /// RTL signal {:?}.", variable.display_name()).unwrap();
            }
            let value_type = public_value_type(width);
            let field_type = if field.array {
                format!("[{value_type}; {}]", field.ids.len())
            } else {
                value_type
            };
            writeln!(source, "    pub {}: {},", field.name, field_type).unwrap();
        }
        writeln!(source, "}}\n").unwrap();
        writeln!(source, "impl Default for {name} {{").unwrap();
        writeln!(source, "    fn default() -> Self {{").unwrap();
        writeln!(source, "        Self {{").unwrap();
        for field in fields {
            let value = if field.array {
                "std::array::from_fn(|_| Default::default())"
            } else {
                "Default::default()"
            };
            writeln!(source, "            {}: {value},", field.name).unwrap();
        }
        writeln!(source, "        }}\n    }}\n}}\n").unwrap();
    }

    fn emit_property_struct(&self, name: &str, kind: PropertyKind, source: &mut String) {
        writeln!(source, "#[derive(Clone, Debug, Default, PartialEq, Eq)]").unwrap();
        writeln!(source, "pub struct {name} {{").unwrap();
        let mut used = BTreeMap::new();
        for variable in &self.design.variables {
            let Some(property) = &variable.property else {
                continue;
            };
            if property.kind != kind {
                continue;
            }
            let field = unique_name(snake_identifier(&property.name), &mut used);
            writeln!(source, "    /// RTL property {:?}.", property.name).unwrap();
            writeln!(source, "    pub {field}: bool,").unwrap();
        }
        writeln!(source, "}}\n").unwrap();
    }

    fn emit_apply_inputs(&self, fields: &[SignalField], source: &mut String) {
        writeln!(source, "fn apply_inputs(env: &mut Env, inputs: &Inputs) {{").unwrap();
        for field in fields {
            for (index, id) in (&field.ids).into_iter().enumerate() {
                let width = self.design.data_type(self.design.variable(*id).dtype).width;
                let value = if field.array {
                    format!("inputs.{}[{index}]", field.name)
                } else {
                    format!("inputs.{}", field.name)
                };
                writeln!(
                    source,
                    "    env.v{} = {};",
                    id.0,
                    public_to_bits(&value, width)
                )
                .unwrap();
            }
        }
        writeln!(source, "}}\n").unwrap();
    }

    fn emit_phase(&mut self, name: &str, statements: &[Statement], source: &mut String) {
        writeln!(source, "fn {name}(env: &mut Env) {{").unwrap();
        writeln!(source, "    let _ = &env;").unwrap();
        for statement in statements {
            self.emit_statement(statement, 1, source);
        }
        writeln!(source, "}}\n").unwrap();
    }

    fn emit_statement(&mut self, statement: &Statement, indent: usize, source: &mut String) {
        let padding = "    ".repeat(indent);
        match &statement.kind {
            StatementKind::Block { statements, .. } => {
                for statement in statements {
                    self.emit_statement(statement, indent, source);
                }
            }
            StatementKind::Assignment { target, value, .. } => {
                let expression = self.emit_expression(value, indent, source, false);
                let (root, offset, width) = self.emit_target(target, indent, source);
                if offset == "0usize"
                    && width
                        == self
                            .design
                            .data_type(self.design.variable(root).dtype)
                            .width
                {
                    let expression_width = self.design.data_type(value.dtype).width;
                    if expression_width == width {
                        writeln!(source, "{padding}env.v{} = {};", root.0, expression).unwrap();
                    } else {
                        let temporary = self.next_temporary();
                        writeln!(
                            source,
                            "{padding}let temp{temporary} = {}.resize::<{width}>(false);",
                            expression
                        )
                        .unwrap();
                        writeln!(source, "{padding}env.v{} = temp{temporary};", root.0).unwrap();
                    }
                } else {
                    writeln!(
                        source,
                        "{padding}env.v{root}.assign_select({offset}, {width}, &{});",
                        expression,
                        root = root.0,
                    )
                    .unwrap();
                }
            }
            StatementKind::If {
                condition,
                then_statements,
                else_statements,
            } => {
                let condition = self.emit_expression(condition, indent, source, false);
                writeln!(source, "{padding}if {condition}.truthy() {{").unwrap();
                for statement in then_statements {
                    self.emit_statement(statement, indent + 1, source);
                }
                writeln!(source, "{padding}}} else {{").unwrap();
                for statement in else_statements {
                    self.emit_statement(statement, indent + 1, source);
                }
                writeln!(source, "{padding}}}").unwrap();
            }
        }
    }

    fn emit_target(
        &mut self,
        target: &AssignmentTarget,
        indent: usize,
        source: &mut String,
    ) -> (VariableId, String, usize) {
        let padding = "    ".repeat(indent);
        match target {
            AssignmentTarget::Variable { variable, .. } => {
                let width = self
                    .design
                    .data_type(self.design.variable(*variable).dtype)
                    .width;
                (*variable, "0usize".to_string(), width)
            }
            AssignmentTarget::Select {
                target,
                offset,
                width,
            } => {
                let (root, base, _) = self.emit_target(target, indent, source);
                let offset = self.emit_expression(offset, indent, source, true);
                let temporary = self.next_temporary();
                writeln!(source, "{padding}let temp{temporary} = {base} + {offset};").unwrap();
                (root, format!("temp{temporary}"), *width)
            }
            AssignmentTarget::ArrayElement {
                array,
                index,
                dtype,
            } => {
                let (root, base, _) = self.emit_target(array, indent, source);
                let layout = self.design.data_type(*dtype).unpacked.as_ref().unwrap();
                let index = self.emit_expression(index, indent, source, true);
                let temporary = self.next_temporary();
                writeln!(
                    source,
                    "{padding}let temp{temporary} = {base} + array_offset({index}, {}, {}, {});",
                    layout.indices.left, layout.indices.right, layout.element_width
                )
                .unwrap();
                (root, format!("temp{temporary}"), layout.element_width)
            }
        }
    }

    fn emit_expression(
        &mut self,
        expression: &Expression,
        indent: usize,
        source: &mut String,
        to_usize: bool,
    ) -> String {
        let dtype = self.design.data_type(expression.dtype);
        let width = dtype.width;
        let padding = "    ".repeat(indent);
        let value = match &expression.kind {
            ExpressionKind::Constant(literal) if to_usize => {
                return literal.value.to_string();
            }
            ExpressionKind::Constant(literal) => generate_constant(literal, width),
            ExpressionKind::Variable { variable, .. } => {
                if let Some(sampled) = &self.design.variable(*variable).sampled_value {
                    return self.emit_expression(sampled, indent, source, to_usize);
                }
                return with_to_usize(format!("env.v{}", variable.0), to_usize);
            }
            ExpressionKind::Unary { operator, operand } => {
                let operand_width = self.design.data_type(operand.dtype).width;
                let operand = self.emit_expression(operand, indent, source, false);
                if (*operator == UnaryOperator::ZeroExtend
                    || *operator == UnaryOperator::SignExtend)
                    && operand_width == width
                {
                    return with_to_usize(operand, to_usize);
                }
                match operator {
                    UnaryOperator::BitwiseNot => format!("{operand}.bit_not()"),
                    UnaryOperator::Negate => format!("{operand}.negate()"),
                    UnaryOperator::ReduceAnd => format!("{operand}.reduce_and()"),
                    UnaryOperator::ReduceOr => format!("{operand}.reduce_or()"),
                    UnaryOperator::ReduceXor => format!("{operand}.reduce_xor()"),
                    UnaryOperator::LogicalNot => {
                        format!("Bits::<1>::from_bool(!{operand}.truthy())")
                    }
                    UnaryOperator::ZeroExtend => format!("{operand}.resize::<{width}>(false)"),
                    UnaryOperator::SignExtend => format!("{operand}.resize::<{width}>(true)"),
                }
            }
            ExpressionKind::Binary { operator, lhs, rhs } => {
                let lhs_value = self.emit_expression(lhs, indent, source, false);
                let rhs_value = self.emit_expression(rhs, indent, source, false);
                self.binary_expression(*operator, &lhs_value, &rhs_value, lhs, rhs, dtype)
            }
            ExpressionKind::Conditional {
                condition,
                then_value,
                else_value,
            } => {
                let condition = self.emit_expression(condition, indent, source, false);
                let temporary = self.next_temporary();
                writeln!(
                    source,
                    "{padding}let temp{temporary} = if {}.truthy() {{",
                    condition
                )
                .unwrap();
                let then_width = self.design.data_type(then_value.dtype).width;
                let then_value = self.emit_expression(then_value, indent + 1, source, false);
                let then_value = resize_owned(then_value, then_width, width, false);
                writeln!(source, "{padding}    {then_value}").unwrap();
                writeln!(source, "{padding}}} else {{").unwrap();
                let else_width = self.design.data_type(else_value.dtype).width;
                let else_value = self.emit_expression(else_value, indent + 1, source, false);
                let else_value = resize_owned(else_value, else_width, width, false);
                writeln!(source, "{padding}    {else_value}").unwrap();
                writeln!(source, "{padding}}};").unwrap();
                return with_to_usize(format!("temp{temporary}"), to_usize);
            }
            ExpressionKind::Replicate {
                source: input,
                count,
                ..
            } => {
                let value = self.emit_expression(input, indent, source, false);
                format!("{value}.replicate::<{width}>({count})")
            }
            ExpressionKind::Select {
                value,
                offset,
                width,
            } => {
                let value = self.emit_expression(value, indent, source, false);
                let offset = self.emit_expression(offset, indent, source, true);
                format!("{value}.select::<{width}>({offset})")
            }
            ExpressionKind::ArraySelect { array, index } => {
                let array_dtype = self.design.data_type(array.dtype);
                let layout = array_dtype.unpacked.as_ref().unwrap();
                let array = self.emit_expression(array, indent, source, false);
                let index = self.emit_expression(index, indent, source, true);
                format!(
                    "{array}.select::<{}>(array_offset({index}, {}, {}, {}))",
                    layout.element_width,
                    layout.indices.left,
                    layout.indices.right,
                    layout.element_width
                )
            }
        };
        let temporary = self.next_temporary();
        writeln!(source, "{padding}let temp{temporary} = {value};").unwrap();
        with_to_usize(format!("temp{temporary}"), to_usize)
    }

    fn binary_expression(
        &mut self,
        operator: BinaryOperator,
        lhs_value: &str,
        rhs_value: &str,
        lhs: &Expression,
        rhs: &Expression,
        dtype: &DataType,
    ) -> String {
        let width = dtype.width;
        let lhs_width = self.design.data_type(lhs.dtype).width;
        let rhs_width = self.design.data_type(rhs.dtype).width;
        match operator {
            BinaryOperator::BitwiseAnd => {
                format!("({lhs_value}).bitwise::<{rhs_width}, {width}>(&({rhs_value}), bool_and)")
            }
            BinaryOperator::BitwiseOr => {
                format!("({lhs_value}).bitwise::<{rhs_width}, {width}>(&({rhs_value}), bool_or)")
            }
            BinaryOperator::BitwiseXor => {
                format!("({lhs_value}).bitwise::<{rhs_width}, {width}>(&({rhs_value}), bool_xor)")
            }
            BinaryOperator::Add | BinaryOperator::Subtract => {
                let method = if operator == BinaryOperator::Add {
                    "add"
                } else {
                    "sub"
                };
                let lhs_value = resize_read(lhs_value, lhs_width, width, dtype.signed);
                let rhs_value = resize_read(rhs_value, rhs_width, width, dtype.signed);
                format!("({lhs_value}).{method}(&({rhs_value}))")
            }
            BinaryOperator::MultiplyUnsigned | BinaryOperator::MultiplySigned => {
                let signed = operator == BinaryOperator::MultiplySigned;
                let lhs_value = resize_read(lhs_value, lhs_width, width, signed);
                let rhs_value = resize_read(rhs_value, rhs_width, width, signed);
                format!("({lhs_value}).mul(&({rhs_value}))")
            }
            BinaryOperator::DivideUnsigned | BinaryOperator::DivideSigned => {
                let signed = operator == BinaryOperator::DivideSigned;
                let method = if signed { "div_signed" } else { "div_unsigned" };
                let lhs_value = resize_read(lhs_value, lhs_width, width, signed);
                let rhs_value = resize_read(rhs_value, rhs_width, width, signed);
                format!("({lhs_value}).{method}(&({rhs_value}))")
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                let comparison = if operator == BinaryOperator::Equal {
                    "=="
                } else {
                    "!="
                };
                let compare_width = lhs_width.max(rhs_width);
                let lhs_value = resize_read(lhs_value, lhs_width, compare_width, false);
                let rhs_value = resize_read(rhs_value, rhs_width, compare_width, false);
                format!("Bits::<1>::from_bool(({lhs_value}) {comparison} ({rhs_value}))")
            }
            BinaryOperator::LessThanUnsigned
            | BinaryOperator::LessThanOrEqualUnsigned
            | BinaryOperator::GreaterThanUnsigned
            | BinaryOperator::GreaterThanOrEqualUnsigned
            | BinaryOperator::LessThanSigned
            | BinaryOperator::LessThanOrEqualSigned
            | BinaryOperator::GreaterThanSigned
            | BinaryOperator::GreaterThanOrEqualSigned => {
                let signed = match operator {
                    BinaryOperator::LessThanSigned
                    | BinaryOperator::LessThanOrEqualSigned
                    | BinaryOperator::GreaterThanSigned
                    | BinaryOperator::GreaterThanOrEqualSigned => true,
                    _ => false,
                };
                let method = if signed { "cmp_signed" } else { "cmp_unsigned" };
                let comparison = match operator {
                    BinaryOperator::LessThanUnsigned | BinaryOperator::LessThanSigned => {
                        "== std::cmp::Ordering::Less"
                    }
                    BinaryOperator::LessThanOrEqualUnsigned
                    | BinaryOperator::LessThanOrEqualSigned => "!= std::cmp::Ordering::Greater",
                    BinaryOperator::GreaterThanUnsigned | BinaryOperator::GreaterThanSigned => {
                        "== std::cmp::Ordering::Greater"
                    }
                    _ => "!= std::cmp::Ordering::Less",
                };
                format!("Bits::<1>::from_bool(({lhs_value}).{method}(&({rhs_value})) {comparison})")
            }
            BinaryOperator::LogicalAnd => {
                format!("Bits::<1>::from_bool(({lhs_value}).truthy() && ({rhs_value}).truthy())")
            }
            BinaryOperator::LogicalOr => {
                format!("Bits::<1>::from_bool(({lhs_value}).truthy() || ({rhs_value}).truthy())")
            }
            BinaryOperator::ShiftLeft => {
                format!("({lhs_value}).shift::<{rhs_width}, {width}>(&({rhs_value}), false, false)")
            }
            BinaryOperator::ShiftRight => {
                format!("({lhs_value}).shift::<{rhs_width}, {width}>(&({rhs_value}), true, false)")
            }
            BinaryOperator::ShiftRightArithmetic => {
                format!("({lhs_value}).shift::<{rhs_width}, {width}>(&({rhs_value}), true, true)")
            }
            BinaryOperator::Concat => {
                format!("Bits::<{width}>::concat(&({lhs_value}), &({rhs_value}))")
            }
        }
    }

    fn emit_make_state(&self, fields: &[SignalField], source: &mut String) {
        writeln!(source, "fn make_state(env: &Env) -> State {{\n    State {{").unwrap();
        for field in fields {
            let values = (&field.ids)
                .into_iter()
                .map(|id| {
                    let width = self.design.data_type(self.design.variable(*id).dtype).width;
                    bits_to_public(&format!("env.v{}", id.0), width)
                })
                .collect::<Vec<_>>();
            if field.array {
                writeln!(source, "        {}: [", field.name).unwrap();
                for value in values {
                    writeln!(source, "            {value},").unwrap();
                }
                writeln!(source, "        ],").unwrap();
            } else {
                writeln!(source, "        {}: {},", field.name, values[0]).unwrap();
            }
        }
        writeln!(source, "    }}\n}}\n").unwrap();
    }

    fn emit_shadow_functions(&self, source: &mut String) {
        writeln!(source, "fn prepare_shadows(env: &mut Env) {{").unwrap();
        writeln!(source, "    let _ = &env;").unwrap();
        for (shadow, register) in &self.design.shadow_registers {
            writeln!(source, "    env.v{} = env.v{};", shadow.0, register.0).unwrap();
        }
        writeln!(source, "}}\n").unwrap();
        writeln!(source, "fn commit_shadows(env: &mut Env) {{").unwrap();
        writeln!(source, "    let _ = &env;").unwrap();
        for (shadow, register) in &self.design.shadow_registers {
            writeln!(source, "    env.v{} = env.v{};", register.0, shadow.0).unwrap();
        }
        writeln!(source, "}}\n").unwrap();
    }

    fn emit_evaluation(&self, output_fields: &[SignalField], source: &mut String) {
        writeln!(source, "fn make_evaluation(env: &Env) -> Evaluation {{").unwrap();
        writeln!(source, "    Evaluation {{").unwrap();
        writeln!(source, "        outputs: Outputs {{").unwrap();
        for field in output_fields {
            let values = (&field.ids)
                .into_iter()
                .map(|id| {
                    let width = self.design.data_type(self.design.variable(*id).dtype).width;
                    bits_to_public(&format!("env.v{}", id.0), width)
                })
                .collect::<Vec<_>>();
            if field.array {
                writeln!(source, "            {}: [", field.name).unwrap();
                for value in values {
                    writeln!(source, "                {value},").unwrap();
                }
                writeln!(source, "            ],").unwrap();
            } else {
                writeln!(source, "            {}: {},", field.name, values[0]).unwrap();
            }
        }
        writeln!(source, "        }},").unwrap();
        for (struct_field, kind, invert) in [
            ("assertions", PropertyKind::Assertion, true),
            ("assumptions", PropertyKind::Assumption, false),
            ("covers", PropertyKind::Cover, false),
        ] {
            let type_name = type_identifier(struct_field);
            writeln!(source, "        {struct_field}: {type_name} {{").unwrap();
            let mut used = BTreeMap::new();
            for (index, variable) in (&self.design.variables).into_iter().enumerate() {
                let Some(property) = &variable.property else {
                    continue;
                };
                if property.kind != kind {
                    continue;
                }
                let field = unique_name(snake_identifier(&property.name), &mut used);
                let prefix = if invert { "!" } else { "" };
                writeln!(
                    source,
                    "            {field}: {prefix}env.v{index}.truthy(),"
                )
                .unwrap();
            }
            writeln!(source, "        }},").unwrap();
        }
        writeln!(source, "    }}\n}}\n").unwrap();
    }

    fn emit_data_types(&self, source: &mut String) {
        for (index, dtype) in (&self.design.data_types).into_iter().enumerate() {
            let Some(name) = &self.type_names[index] else {
                continue;
            };
            let width = dtype.width;
            match &dtype.kind {
                DataTypeKind::Basic { .. } => {
                    writeln!(
                        source,
                        "/// RTL datatype {:?}.",
                        dtype.name.as_deref().unwrap_or("anonymous")
                    )
                    .unwrap();
                    writeln!(source, "pub type {name} = {};\n", public_value_type(width)).unwrap();
                }
                DataTypeKind::Alias { target } => {
                    let target = self
                        .type_names
                        .get(target.0)
                        .and_then(Clone::clone)
                        .unwrap_or_else(|| public_value_type(self.design.data_type(*target).width));
                    writeln!(
                        source,
                        "/// RTL type alias {:?}.",
                        dtype.name.as_deref().unwrap_or("anonymous")
                    )
                    .unwrap();
                    writeln!(source, "pub type {name} = {target};\n").unwrap();
                }
                DataTypeKind::Enum { variants, .. } => {
                    let raw = public_value_type(width);
                    writeln!(
                        source,
                        "/// Lossless representation of RTL enum {:?}.",
                        dtype.name.as_deref().unwrap_or("anonymous")
                    )
                    .unwrap();
                    writeln!(source, "#[derive(Clone, Debug, Default, PartialEq, Eq)]\npub struct {name}(pub {raw});").unwrap();
                    writeln!(source, "impl {name} {{").unwrap();
                    for variant in variants {
                        let variant_name = screaming_identifier(&variant.name);
                        if width <= 128 {
                            let value = if width <= 1 {
                                format!("{} != 0", variant.value.value)
                            } else {
                                format!("{} as {raw}", variant.value.value)
                            };
                            writeln!(
                                source,
                                "    pub const {variant_name}: Self = Self({value});"
                            )
                            .unwrap();
                        } else {
                            writeln!(source, "    pub fn {variant_name}() -> Self {{ Self(Bits::from_words(&{:?})) }}", variant.value.value.to_u64_digits()).unwrap();
                        }
                    }
                    writeln!(source, "}}\n").unwrap();
                }
                DataTypeKind::PackedArray { declared, .. } => {
                    let element_width = width / declared.len();
                    writeln!(
                        source,
                        "/// Lossless packed representation of RTL datatype {rtl_name:?}.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct {name}(pub Bits<{width}>);


impl {name} {{
    pub fn get(&self, index: usize) -> Bits<{element_width}> {{
        self.0.select::<{element_width}>(array_offset(index, {left}, {right}, {element_width}))
    }}
    pub fn set(&mut self, index: usize, value: Bits<{element_width}>) {{
        let offset = array_offset(index, {left}, {right}, {element_width});
        self.0.assign_select(offset, {element_width}, &value);
    }}
}}
",
                        rtl_name = dtype.name.as_deref().unwrap_or("anonymous"),
                        left = declared.left,
                        right = declared.right,
                    )
                    .unwrap();
                }
                DataTypeKind::UnpackedArray { .. } => {
                    let layout = dtype.unpacked.as_ref().unwrap();
                    writeln!(
                        source,
                        "/// Lossless packed representation of RTL datatype {rtl_name:?}.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct {name}(pub Bits<{width}>);


impl {name} {{
    pub fn get(&self, index: usize) -> Bits<{element_width}> {{
        self.0.select::<{element_width}>(array_offset(index, {left}, {right}, {element_width}))
    }}
    pub fn set(&mut self, index: usize, value: Bits<{element_width}>) {{
        let offset = array_offset(index, {left}, {right}, {element_width});
        self.0.assign_select(offset, {element_width}, &value);
    }}
}}
",
                        rtl_name = dtype.name.as_deref().unwrap_or("anonymous"),
                        element_width = layout.element_width,
                        left = layout.indices.left,
                        right = layout.indices.right,
                    )
                    .unwrap();
                }
                DataTypeKind::PackedStruct { members } | DataTypeKind::PackedUnion { members } => {
                    writeln!(
                        source,
                        "/// Lossless packed representation of RTL datatype {rtl_name:?}.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct {name}(pub Bits<{width}>);

impl {name} {{",
                        rtl_name = dtype.name.as_deref().unwrap_or("anonymous"),
                    )
                    .unwrap();
                    let member_names = unique_names(
                        members
                            .into_iter()
                            .map(|member| snake_identifier(&member.name)),
                    );
                    for (member, member_name) in members.into_iter().zip(member_names) {
                        writeln!(
                            source,
                            "
    pub fn {member_name}(&self) -> Bits<{width}> {{
        self.0.select::<{width}>({lsb})
    }}
    pub fn set_{member_name}(&mut self, value: Bits<{width}>) {{
        self.0.assign_select({lsb}, {width}, &value);
    }}
",
                            width = member.width,
                            lsb = member.lsb,
                        )
                        .unwrap();
                    }
                    writeln!(source, "}}\n").unwrap();
                }
            }
        }
    }

    fn next_temporary(&mut self) -> usize {
        let temporary = self.temporary;
        self.temporary += 1;
        temporary
    }
}

fn generate_constant(literal: &Literal, width: usize) -> String {
    let words = literal.value.to_u64_digits();
    if width < usize::BITS as usize {
        let value = match words.as_slice() {
            [] => 0,
            [value] => usize::try_from(*value).expect("narrow constant exceeds usize"),
            _ => panic!("narrow constant exceeds usize"),
        };
        format!("Bits::<{width}>::from_usize({value})")
    } else {
        format!("Bits::<{width}>::from_words(&{words:?})")
    }
}

fn indexed_field_name(name: &str) -> Option<(&str, usize)> {
    let (base, index) = name.rsplit_once("_v")?;
    if base.is_empty() || index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((base, index.parse().ok()?))
}

fn order_combinational(design: &Design) -> (Vec<Statement>, bool) {
    let mut writes: BTreeSet<VariableId> = BTreeSet::new();
    let mut statement_accesses = Vec::new();
    for statement in &design.combinational {
        let mut reads = BTreeSet::new();
        let mut local_writes = BTreeSet::new();
        statement.collect_accesses(&mut reads, &mut local_writes);
        writes.extend(&local_writes);
        statement_accesses.push((statement, reads, local_writes));
    }
    let mut available = (0..design.variables.len())
        .map(VariableId)
        .filter(|id| !writes.contains(id))
        .collect::<BTreeSet<_>>();
    let mut pending = statement_accesses;
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let Some(index) = (&pending)
            .into_iter()
            .position(|(_, reads, _)| reads.into_iter().all(|read| available.contains(read)))
        else {
            ordered.extend(
                pending
                    .into_iter()
                    .map(|(statement, _, _)| statement.clone()),
            );
            return (ordered, true);
        };
        let (statement, _, local_writes) = pending.remove(index);
        available.extend(local_writes);
        ordered.push(statement.clone());
    }
    (ordered, false)
}
