mod dtype;
mod expression;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    ast::{
        AccessMode, AssignmentKind, AssignmentTarget, BlockKind, DataType, DataTypeId, Design,
        Direction, Domain, Edge, Expression, ExpressionKind, Property, PropertyKind, SignalDomain,
        SourceInfo, Statement, StatementKind, Variable, VariableId, VariableKind,
    },
    document::{AstDocument, AstNode},
    parser::{dtype::DataTypeResolver, expression::is_supported_expression},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(String);

impl ParseError {
    fn node(node: &AstNode, message: impl fmt::Display) -> Self {
        Self(format!(
            "{} at {}: {message}",
            node.node_type,
            node.loc.as_deref().unwrap_or("unknown location")
        ))
    }

    fn message(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ParseError {}

struct Parser<'a> {
    document: &'a AstDocument,
    nodes_by_addr: BTreeMap<String, &'a AstNode>,
    dtype_ids: BTreeMap<String, DataTypeId>,
    data_types: Vec<DataType>,
    variable_ids: BTreeMap<String, VariableId>,
    variables: Vec<Variable>,
}

impl TryFrom<&AstDocument> for Design {
    type Error = ParseError;

    fn try_from(document: &AstDocument) -> Result<Self, Self::Error> {
        Parser::new(document)?.parse()
    }
}

impl<'a> Parser<'a> {
    fn new(document: &'a AstDocument) -> Result<Self, ParseError> {
        let mut nodes_by_addr = BTreeMap::new();
        for node in document.nodes() {
            if let Some(address) = &node.addr {
                nodes_by_addr.entry(address.clone()).or_insert(node);
            }
        }

        let mut dtype_resolver = DataTypeResolver::new(&nodes_by_addr);
        dtype_resolver.resolve_supported_declarations();
        let scopes = find_scopes(document, &nodes_by_addr)?;
        let mut variable_ids = BTreeMap::new();
        let mut variable_nodes = Vec::new();
        for scope in &scopes {
            let is_root = scope.string("name") == Some("TOP");
            for var_scope in scope.children("varsp") {
                if var_scope.node_type != "VARSCOPE" {
                    return Err(ParseError::node(var_scope, "expected VARSCOPE under SCOPE"));
                }
                let address = var_scope
                    .addr
                    .as_ref()
                    .ok_or_else(|| ParseError::node(var_scope, "variable scope has no addr"))?;
                let declaration_address = var_scope
                    .string("varp")
                    .ok_or_else(|| ParseError::node(var_scope, "variable scope has no varp"))?;
                let declaration =
                    nodes_by_addr
                        .get(declaration_address)
                        .copied()
                        .ok_or_else(|| {
                            ParseError::node(
                                var_scope,
                                format!("unresolved varp {declaration_address}"),
                            )
                        })?;
                if declaration.node_type != "VAR" {
                    return Err(ParseError::node(
                        declaration,
                        "VARSCOPE varp does not reference VAR",
                    ));
                }
                let id = VariableId(variable_nodes.len());
                variable_ids.insert(address.clone(), id);
                variable_nodes.push((var_scope, declaration, is_root));
            }
        }

        if variable_nodes.is_empty() {
            for node in document.nodes() {
                if node.node_type != "VAR" {
                    continue;
                }
                let address = node
                    .addr
                    .as_ref()
                    .ok_or_else(|| ParseError::node(node, "variable has no addr"))?;
                if variable_ids.contains_key(address) {
                    continue;
                }
                let id = VariableId(variable_nodes.len());
                variable_ids.insert(address.clone(), id);
                variable_nodes.push((node, node, true));
            }
        }

        let mut variables = Vec::with_capacity(variable_nodes.len());
        for (var_scope, declaration, is_root) in variable_nodes {
            let dtype_address = declaration
                .string("dtypep")
                .ok_or_else(|| ParseError::node(declaration, "variable has no dtypep"))?;
            let dtype = dtype_resolver.resolve(dtype_address, declaration)?;
            let direction = parse_direction(declaration)?;
            let kind = parse_variable_kind(declaration)?;
            let name = var_scope
                .name
                .clone()
                .unwrap_or_else(|| "unnamed".to_string());
            let original_name = if is_root {
                declaration.string("origName").map(str::to_string)
            } else {
                Some(name.clone())
            };
            let internal = is_internal_name(&name, original_name.as_deref().unwrap_or(""));
            let mut property = declaration.string("origName").and_then(parse_sva_name);
            if !is_root && let Some(property) = &mut property {
                let scope_name = var_scope
                    .string("scopep")
                    .and_then(|address| nodes_by_addr.get(address))
                    .and_then(|scope| scope.string("name"))
                    .unwrap_or("unnamed");
                property.name = format!("{scope_name}.{}", property.name);
            }
            variables.push(Variable {
                source: source_info(var_scope),
                name,
                original_name,
                dtype,
                direction,
                kind,
                top_level: is_root,
                internal,
                sampled_value: None,
                property,
            });
        }
        for node in document.nodes() {
            if !is_supported_expression(&node.node_type) {
                continue;
            }
            if let Some(dtype_address) = node.string("dtypep") {
                dtype_resolver.resolve(dtype_address, node)?;
            }
        }
        let (dtype_ids, data_types) = dtype_resolver.finish();

        Ok(Self {
            document,
            nodes_by_addr,
            dtype_ids,
            data_types,
            variable_ids,
            variables,
        })
    }

    fn parse(mut self) -> Result<Design, ParseError> {
        for node in self.document.nodes() {
            let (id, declaration) = if node.node_type == "VARSCOPE" {
                let Some(address) = &node.addr else {
                    continue;
                };
                let Some(id) = self.variable_ids.get(address).copied() else {
                    continue;
                };
                let declaration_address = node
                    .string("varp")
                    .ok_or_else(|| ParseError::node(node, "variable scope has no varp"))?;
                let declaration = self
                    .nodes_by_addr
                    .get(declaration_address)
                    .copied()
                    .ok_or_else(|| {
                        ParseError::node(node, format!("unresolved varp {declaration_address}"))
                    })?;
                (id, declaration)
            } else if node.node_type == "VAR" {
                let Some(address) = &node.addr else {
                    continue;
                };
                let Some(id) = self.variable_ids.get(address).copied() else {
                    continue;
                };
                (id, node)
            } else {
                continue;
            };
            if !declaration.boolean("sampled") {
                continue;
            }
            let value = declaration
                .child("valuep")
                .ok_or_else(|| ParseError::node(declaration, "sampled variable has no valuep"))?;
            let expression = self.parse_expression(value)?;
            self.variables[id.0].sampled_value = Some(expression);
        }

        let active_blocks = find_scopes(self.document, &self.nodes_by_addr)?
            .into_iter()
            .flat_map(|scope| scope.children("blocksp"))
            .collect::<Vec<_>>();
        let sensitivity_domains = self.parse_sensitivity_domains(&active_blocks)?;
        let shadow_registers = self.parse_shadow_registers(&active_blocks)?;

        let mut static_initial = Vec::new();
        let mut initial = Vec::new();
        let mut combinational = Vec::new();
        let mut pre_edge = Vec::new();
        let mut sequential = Vec::new();
        let mut post_edge = Vec::new();
        for active in &active_blocks {
            match active.string("name") {
                None | Some("") => {
                    for statement in active.children("stmtsp") {
                        if statement.node_type == "INITIALSTATIC" {
                            static_initial.push(self.parse_statement(statement)?);
                        } else if statement.node_type == "INITIAL" {
                            initial.push(self.parse_statement(statement)?);
                        } else {
                            combinational.push(self.parse_statement(statement)?);
                        }
                    }
                }
                Some("sequent") => {
                    sequential.extend(self.parse_statements(active.children("stmtsp"))?);
                }
                Some("nba-flag-shared") => {
                    for phase in active.children("stmtsp") {
                        match phase.node_type.as_str() {
                            "ALWAYSPRE" => pre_edge.push(self.parse_statement(phase)?),
                            "ALWAYSPOST" => post_edge.push(self.parse_statement(phase)?),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        if sequential.is_empty() {
            return Err(ParseError::message("AST has no sequent ACTIVE block"));
        }

        static_initial.extend(initial);
        Ok(Design {
            source: source_info(&self.document.root),
            data_types: self.data_types,
            variables: self.variables,
            sensitivity_domains,
            shadow_registers,
            initial: static_initial,
            combinational,
            pre_edge,
            sequential,
            post_edge,
        })
    }

    fn parse_sensitivity_domains(
        &self,
        active_blocks: &[&AstNode],
    ) -> Result<Vec<SignalDomain>, ParseError> {
        let mut domains = BTreeSet::new();
        for active in active_blocks {
            if active.string("name") != Some("sequent") {
                continue;
            }
            let Some(sentree_address) = active.string("sentreep") else {
                continue;
            };
            let sentree = self
                .nodes_by_addr
                .get(sentree_address)
                .copied()
                .ok_or_else(|| {
                    ParseError::node(active, format!("unresolved sentreep {sentree_address}"))
                })?;
            let senses = sentree.children("sensesp");
            if senses.is_empty() {
                return Err(ParseError::node(sentree, "unsupported sensitivity tree"));
            }

            for sense in senses {
                if sense.child("condp").is_some() {
                    return Err(ParseError::node(sentree, "unsupported sensitivity tree"));
                }
                let signal = sense
                    .child("sensp")
                    .ok_or_else(|| ParseError::node(sense, "sensitivity item has no signal"))?;
                let variable = self.reference_variable_id(signal)?;
                let edge = parse_edge(sense)?;
                domains.insert((variable, edge));
            }
        }
        Ok(domains
            .into_iter()
            .map(|(variable, edge)| SignalDomain {
                variable,
                domain: Domain::new(self.variables[variable.0].display_name(), edge),
            })
            .collect())
    }

    fn parse_shadow_registers(
        &self,
        active_blocks: &[&AstNode],
    ) -> Result<Vec<(VariableId, VariableId)>, ParseError> {
        let mut pre = BTreeMap::new();
        let mut post = BTreeMap::new();
        for active in active_blocks {
            if active.string("name") != Some("nba-shadow-variable") {
                continue;
            }
            for phase in active.children("stmtsp") {
                for assignment in phase.children("stmtsp") {
                    if assignment.node_type != "ASSIGN" {
                        return Err(ParseError::node(
                            assignment,
                            "unsupported NBA phase statement",
                        ));
                    }
                    let lhs = self.assignment_variable(assignment, "lhsp")?;
                    let rhs = self.assignment_variable(assignment, "rhsp")?;
                    if phase.node_type == "ALWAYSPRE" {
                        pre.insert(lhs, rhs);
                    } else if phase.node_type == "ALWAYSPOST" {
                        post.insert(rhs, lhs);
                    }
                }
            }
        }
        if pre != post {
            return Err(ParseError::message(
                "ALWAYSPRE and ALWAYSPOST shadow/register pairs disagree",
            ));
        }
        Ok(pre.into_iter().collect())
    }

    fn assignment_variable(
        &self,
        statement: &AstNode,
        field: &str,
    ) -> Result<VariableId, ParseError> {
        let reference = statement
            .child(field)
            .ok_or_else(|| ParseError::node(statement, format!("assignment has no {field}")))?;
        if reference.node_type != "VARREF" {
            return Err(ParseError::node(
                reference,
                "expected whole-variable VARREF",
            ));
        }
        self.reference_variable_id(reference)
    }

    fn parse_statements(&self, nodes: Vec<&AstNode>) -> Result<Vec<Statement>, ParseError> {
        nodes
            .into_iter()
            .map(|node| self.parse_statement(node))
            .collect()
    }

    fn parse_statement(&self, node: &AstNode) -> Result<Statement, ParseError> {
        let kind = match node.node_type.as_str() {
            "INITIAL" | "INITIALSTATIC" => StatementKind::Block {
                kind: BlockKind::Initial,
                statements: self.parse_statements(node.children("stmtsp"))?,
            },
            "ALWAYS" => StatementKind::Block {
                kind: BlockKind::Always,
                statements: self.parse_statements(node.children("stmtsp"))?,
            },
            "ALWAYSPRE" => StatementKind::Block {
                kind: BlockKind::AlwaysPre,
                statements: self.parse_statements(node.children("stmtsp"))?,
            },
            "ALWAYSPOST" => StatementKind::Block {
                kind: BlockKind::AlwaysPost,
                statements: self.parse_statements(node.children("stmtsp"))?,
            },
            "ASSIGN" | "ASSIGNDLY" | "ASSIGNW" => {
                let lhs = node
                    .child("lhsp")
                    .ok_or_else(|| ParseError::node(node, "assignment has no lhs"))?;
                let rhs = node
                    .child("rhsp")
                    .ok_or_else(|| ParseError::node(node, "assignment has no rhs"))?;
                let target = self.parse_target(lhs)?;
                let value = self.parse_expression(rhs)?;
                validate_assignment(node, &target, &value, &self.variables, &self.data_types)?;
                StatementKind::Assignment {
                    kind: match node.node_type.as_str() {
                        "ASSIGN" => AssignmentKind::Blocking,
                        "ASSIGNDLY" => AssignmentKind::Delayed,
                        "ASSIGNW" => AssignmentKind::Continuous,
                        _ => unreachable!(),
                    },
                    target,
                    value,
                }
            }
            "IF" => {
                let condition = node
                    .child("condp")
                    .ok_or_else(|| ParseError::node(node, "IF has no condition"))?;
                StatementKind::If {
                    condition: self.parse_expression(condition)?,
                    then_statements: self.parse_statements(node.children("thensp"))?,
                    else_statements: self.parse_statements(node.children("elsesp"))?,
                }
            }
            _ => {
                return Err(ParseError::node(node, "unsupported procedural statement"));
            }
        };
        Ok(Statement {
            source: source_info(node),
            kind,
        })
    }

    fn parse_target(&self, node: &AstNode) -> Result<AssignmentTarget, ParseError> {
        match node.node_type.as_str() {
            "VARREF" => Ok(AssignmentTarget::Variable {
                variable: self.reference_variable_id(node)?,
                access: parse_access(node)?,
            }),
            "SEL" => {
                let from = node
                    .child("fromp")
                    .ok_or_else(|| ParseError::node(node, "SEL assignment has no fromp"))?;
                if from.node_type != "VARREF" && from.node_type != "ARRAYSEL" {
                    return Err(ParseError::node(
                        from,
                        "SEL assignment base must be a VARREF or ARRAYSEL",
                    ));
                }
                let target = Box::new(self.parse_target(from)?);
                let offset_node = node
                    .child("lsbp")
                    .ok_or_else(|| ParseError::node(node, "SEL has no lsbp"))?;
                let offset = self.parse_expression(offset_node)?;
                let width = self.selection_width(node)?;
                let target_width =
                    assignment_target_width(&target, &self.variables, &self.data_types);
                if width > target_width {
                    return Err(ParseError::node(node, "SEL assignment is out of range"));
                }
                if let ExpressionKind::Constant(literal) = &offset.kind {
                    let offset = usize::try_from(&literal.value).map_err(|_| {
                        ParseError::node(offset_node, "selection constant exceeds usize")
                    })?;
                    if offset
                        .checked_add(width)
                        .is_none_or(|end| end > target_width)
                    {
                        return Err(ParseError::node(node, "SEL assignment is out of range"));
                    }
                }
                Ok(AssignmentTarget::Select {
                    target,
                    offset,
                    width,
                })
            }
            "ARRAYSEL" => {
                let from = node
                    .child("fromp")
                    .ok_or_else(|| ParseError::node(node, "ARRAYSEL assignment has no fromp"))?;
                if from.node_type != "VARREF" && from.node_type != "ARRAYSEL" {
                    return Err(ParseError::node(
                        from,
                        "ARRAYSEL assignment base must be a VARREF or ARRAYSEL",
                    ));
                }
                let array = Box::new(self.parse_target(from)?);
                let index = node
                    .child("bitp")
                    .ok_or_else(|| ParseError::node(node, "ARRAYSEL has no bitp"))?;
                let dtype = self.node_dtype(from)?;
                if self.data_types[dtype.0].unpacked.is_none() {
                    return Err(ParseError::node(
                        from,
                        "ARRAYSEL target is not an unpacked array",
                    ));
                }
                Ok(AssignmentTarget::ArrayElement {
                    array,
                    index: self.parse_expression(index)?,
                    dtype,
                })
            }
            _ => Err(ParseError::node(node, "unsupported assignment target")),
        }
    }

    fn variable_id(&self, node: &AstNode, address: &str) -> Result<VariableId, ParseError> {
        self.variable_ids
            .get(address)
            .copied()
            .ok_or_else(|| ParseError::node(node, format!("unresolved varp {address}")))
    }

    fn reference_variable_id(&self, node: &AstNode) -> Result<VariableId, ParseError> {
        let address = node
            .string("varScopep")
            .filter(|address| *address != "UNLINKED")
            .or_else(|| node.string("varp"))
            .ok_or_else(|| ParseError::node(node, "VARREF has no varScopep or varp"))?;
        self.variable_id(node, address)
    }
}

fn find_scopes<'a>(
    document: &'a AstDocument,
    nodes_by_addr: &BTreeMap<String, &'a AstNode>,
) -> Result<Vec<&'a AstNode>, ParseError> {
    let Some(top_scope_address) = document.root.string("topScopep") else {
        return find_legacy_scope(nodes_by_addr).map(|scope| vec![scope]);
    };
    let top_scope = nodes_by_addr
        .get(top_scope_address)
        .copied()
        .ok_or_else(|| {
            ParseError::node(
                &document.root,
                format!("unresolved topScopep {top_scope_address}"),
            )
        })?;
    if top_scope.node_type != "TOPSCOPE" {
        return Err(ParseError::node(
            top_scope,
            "topScopep does not reference TOPSCOPE",
        ));
    }

    let roots = top_scope.children("scopep");
    if roots.is_empty() {
        return Err(ParseError::node(top_scope, "TOPSCOPE has no SCOPE"));
    }
    let mut scope_addresses = BTreeSet::new();
    for scope in roots {
        if scope.node_type != "SCOPE" {
            return Err(ParseError::node(scope, "expected SCOPE under TOPSCOPE"));
        }
        let address = scope
            .addr
            .as_ref()
            .ok_or_else(|| ParseError::node(scope, "scope has no addr"))?;
        scope_addresses.insert(address.clone());
    }

    loop {
        let mut changed = false;
        for scope in document.nodes() {
            if scope.node_type != "SCOPE" {
                continue;
            }
            let Some(parent) = scope.string("aboveScopep") else {
                continue;
            };
            if !scope_addresses.contains(parent) {
                continue;
            }
            let address = scope
                .addr
                .as_ref()
                .ok_or_else(|| ParseError::node(scope, "scope has no addr"))?;
            changed |= scope_addresses.insert(address.clone());
        }
        if !changed {
            break;
        }
    }

    Ok(document
        .nodes()
        .into_iter()
        .filter(|node| {
            node.node_type == "SCOPE"
                && node
                    .addr
                    .as_ref()
                    .is_some_and(|address| scope_addresses.contains(address))
        })
        .collect())
}

fn find_legacy_scope<'a>(
    nodes_by_addr: &BTreeMap<String, &'a AstNode>,
) -> Result<&'a AstNode, ParseError> {
    nodes_by_addr
        .values()
        .find(|node| {
            node.node_type == "SCOPE"
                && node.string("name") == Some("TOP")
                && !node.children("blocksp").is_empty()
        })
        .copied()
        .or_else(|| {
            nodes_by_addr
                .values()
                .find(|node| node.node_type == "SCOPE" && node.string("name") == Some("TOP"))
                .copied()
        })
        .or_else(|| {
            nodes_by_addr
                .values()
                .find(|node| node.node_type == "SCOPE")
                .copied()
        })
        .ok_or_else(|| ParseError::message("AST has no SCOPE"))
}

fn parse_edge(sense: &AstNode) -> Result<Edge, ParseError> {
    match sense.string("edgeType") {
        Some("POS") => Ok(Edge::Positive),
        Some("NEG") => Ok(Edge::Negative),
        _ => Err(ParseError::node(sense, "only posedge/negedge is supported")),
    }
}

fn parse_direction(node: &AstNode) -> Result<Direction, ParseError> {
    match node.string("direction").unwrap_or("NONE") {
        "INPUT" => Ok(Direction::Input),
        "OUTPUT" => Ok(Direction::Output),
        "NONE" => Ok(Direction::None),
        direction => Err(ParseError::node(
            node,
            format!("unsupported variable direction {direction}"),
        )),
    }
}

fn parse_variable_kind(node: &AstNode) -> Result<VariableKind, ParseError> {
    match node.string("varType").unwrap_or("VAR") {
        "PORT" => Ok(VariableKind::Port),
        "MODULETEMP" => Ok(VariableKind::ModuleTemporary),
        "VAR" => Ok(VariableKind::Variable),
        "WIRE" => Ok(VariableKind::Wire),
        "NONE" | "BLOCKTEMP" | "GPARAM" | "LPARAM" => Ok(VariableKind::Other),
        kind => Err(ParseError::node(
            node,
            format!("unsupported variable type {kind}"),
        )),
    }
}

fn parse_access(node: &AstNode) -> Result<AccessMode, ParseError> {
    match node.string("access").unwrap_or("RD") {
        "RD" => Ok(AccessMode::Read),
        "WR" => Ok(AccessMode::Write),
        access => Err(ParseError::node(
            node,
            format!("unsupported variable access mode {access}"),
        )),
    }
}

fn validate_assignment(
    node: &AstNode,
    target: &AssignmentTarget,
    value: &Expression,
    variables: &[Variable],
    data_types: &[DataType],
) -> Result<(), ParseError> {
    let value_width = data_types[value.dtype.0].width;
    let target_width = match target {
        AssignmentTarget::Variable { variable, .. } => {
            data_types[variables[variable.0].dtype.0].width
        }
        AssignmentTarget::Select { width, .. } => *width,
        AssignmentTarget::ArrayElement { dtype, .. } => {
            data_types[dtype.0].unpacked.as_ref().unwrap().element_width
        }
    };
    let requires_exact_width = match target {
        AssignmentTarget::Variable { .. } => false,
        AssignmentTarget::Select { .. } | AssignmentTarget::ArrayElement { .. } => true,
    };
    if requires_exact_width && value_width != target_width {
        return Err(ParseError::node(node, "assignment width mismatch"));
    }
    Ok(())
}

fn assignment_target_width(
    target: &AssignmentTarget,
    variables: &[Variable],
    data_types: &[DataType],
) -> usize {
    match target {
        AssignmentTarget::Variable { variable, .. } => {
            data_types[variables[variable.0].dtype.0].width
        }
        AssignmentTarget::Select { width, .. } => *width,
        AssignmentTarget::ArrayElement { dtype, .. } => {
            data_types[dtype.0].unpacked.as_ref().unwrap().element_width
        }
    }
}

fn parse_sva_name(original_name: &str) -> Option<Property> {
    for (prefix, kind) in [
        ("__Vsva_assert_", PropertyKind::Assertion),
        ("__Vsva_assume_", PropertyKind::Assumption),
        ("__Vsva_cover_", PropertyKind::Cover),
    ] {
        if let Some(name) = original_name.strip_prefix(prefix) {
            let name = name
                .rsplit_once("__")
                .map_or(name, |(property_name, _)| property_name);
            return Some(Property {
                kind,
                name: name.to_string(),
            });
        }
    }
    None
}

fn is_internal_name(name: &str, original_name: &str) -> bool {
    name.starts_with("__V")
        || name.contains(".__V")
        || original_name.starts_with("__V")
        || original_name.starts_with("_Vpast_")
        || original_name.contains("._Vpast_")
}

fn source_info(node: &AstNode) -> SourceInfo {
    SourceInfo {
        node_type: node.node_type.clone(),
        address: node.addr.clone(),
        location: node.loc.clone(),
    }
}
