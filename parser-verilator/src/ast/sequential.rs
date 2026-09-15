use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    AssignmentKind, AssignmentTarget, Design, Direction, ExpressionKind, Statement, StatementKind,
    VariableId, collect::CollectAccesses,
};

/// State and NBA destinations in the supported single-edge execution model.
#[derive(Debug, Default)]
pub struct SequentialInfo {
    pub registers: BTreeSet<VariableId>,
    pub nonblocking: BTreeSet<VariableId>,
}

/// Analyze original clocked processes, without Verilator's NBA lowering artifacts.
/// Mixed assignment kinds and races between processes are deliberately unsupported.
pub fn analyze(design: &Design) -> Result<SequentialInfo, String> {
    let mut info = SequentialInfo::default();
    let mut owners = BTreeMap::new();
    let mut blocking = BTreeSet::new();
    let mut process_reads = Vec::new();
    for (index, process) in (&design.sequential).into_iter().enumerate() {
        let mut reads = BTreeSet::new();
        let mut writes = BTreeSet::new();
        process.collect_accesses(&mut reads, &mut writes);
        let mut footprints = BTreeMap::new();
        collect_footprints(design, process, &mut footprints);
        for (id, bits) in footprints {
            let previous = owners.entry(id).or_insert_with(BTreeMap::new);
            for bit in bits {
                if previous.insert(bit, index).is_some() {
                    return Err(format!(
                        "multiple clocked processes write overlapping bits of {}",
                        design.variable(id).display_name()
                    ));
                }
            }
        }
        process_reads.push(reads);
        let mut assigned = BTreeSet::new();
        let mut live_in = BTreeSet::new();
        analyze_statement(
            process,
            &mut assigned,
            &mut live_in,
            &mut blocking,
            &mut info.nonblocking,
        );
        info.registers.extend(live_in.intersection(&writes));
    }
    if let Some(id) = blocking.intersection(&info.nonblocking).next() {
        return Err(format!(
            "mixed blocking and nonblocking writes to {} are unsupported",
            design.variable(*id).display_name()
        ));
    }
    let mut outside_reads = BTreeSet::new();
    let mut combinational_writes = BTreeSet::new();
    design
        .combinational
        .as_slice()
        .collect_accesses(&mut outside_reads, &mut combinational_writes);
    for id in owners.keys() {
        if combinational_writes.contains(id) {
            return Err(format!(
                "combinational and clocked processes both write {}",
                design.variable(*id).display_name()
            ));
        }
    }
    for variable in &design.variables {
        if let Some(sampled) = &variable.sampled_value {
            sampled.collect_reads(&mut outside_reads);
        }
    }
    for id in &blocking {
        for (index, reads) in (&process_reads).into_iter().enumerate() {
            if owners[id].values().any(|owner| *owner != index) && reads.contains(id) {
                return Err(format!(
                    "cross-process read of blocking-written {} is unsupported",
                    design.variable(*id).display_name()
                ));
            }
        }
        if outside_reads.contains(id) || design.variable(*id).direction == Direction::Output {
            info.registers.insert(*id);
        }
    }
    info.registers.extend(&info.nonblocking);
    for statement in (&design.initial).into_iter().chain(&design.combinational) {
        let mut nba = BTreeSet::new();
        analyze_statement(
            statement,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut nba,
        );
        if !nba.is_empty() {
            return Err("nonblocking assignments outside clocked processes are unsupported".into());
        }
    }
    Ok(info)
}

fn analyze_statement(
    statement: &Statement,
    assigned: &mut BTreeSet<VariableId>,
    live_in: &mut BTreeSet<VariableId>,
    blocking: &mut BTreeSet<VariableId>,
    nonblocking: &mut BTreeSet<VariableId>,
) {
    match &statement.kind {
        StatementKind::Block { statements, .. } => {
            for statement in statements {
                analyze_statement(statement, assigned, live_in, blocking, nonblocking);
            }
        }
        StatementKind::Assignment {
            kind,
            target,
            value,
        } => {
            let mut reads = BTreeSet::new();
            let mut writes = BTreeSet::new();
            target.collect_accesses(&mut reads, &mut writes);
            value.collect_reads(&mut reads);
            live_in.extend(reads.difference(assigned));
            if *kind == AssignmentKind::Nonblocking {
                nonblocking.extend(writes);
            } else {
                blocking.extend(writes);
                if let AssignmentTarget::Variable { variable, .. } = target {
                    assigned.insert(*variable);
                }
            }
        }
        StatementKind::If {
            condition,
            then_statements,
            else_statements,
        } => {
            let mut reads = BTreeSet::new();
            condition.collect_reads(&mut reads);
            live_in.extend(reads.difference(assigned));
            let mut then_assigned = assigned.clone();
            let mut else_assigned = assigned.clone();
            for statement in then_statements {
                analyze_statement(
                    statement,
                    &mut then_assigned,
                    live_in,
                    blocking,
                    nonblocking,
                );
            }
            for statement in else_statements {
                analyze_statement(
                    statement,
                    &mut else_assigned,
                    live_in,
                    blocking,
                    nonblocking,
                );
            }
            *assigned = then_assigned
                .intersection(&else_assigned)
                .copied()
                .collect();
        }
    }
}

// Track possible written bits rather than whole variables: generated RAMs often
// have one process per bit lane, with dynamic addresses but disjoint lanes.
fn collect_footprints(
    design: &Design,
    statement: &Statement,
    writes: &mut BTreeMap<VariableId, BTreeSet<usize>>,
) {
    match &statement.kind {
        StatementKind::Block { statements, .. } => {
            for statement in statements {
                collect_footprints(design, statement, writes);
            }
        }
        StatementKind::If {
            then_statements,
            else_statements,
            ..
        } => {
            for statement in then_statements.into_iter().chain(else_statements) {
                collect_footprints(design, statement, writes);
            }
        }
        StatementKind::Assignment { target, .. } => {
            let (id, regions) = target_regions(design, target);
            let bits = writes.entry(id).or_default();
            for (start, width) in regions {
                bits.extend(start..start + width);
            }
        }
    }
}

fn target_regions(design: &Design, target: &AssignmentTarget) -> (VariableId, Vec<(usize, usize)>) {
    match target {
        AssignmentTarget::Variable { variable, .. } => (
            *variable,
            vec![(0, design.data_type(design.variable(*variable).dtype).width)],
        ),
        AssignmentTarget::Select {
            target,
            offset,
            width,
        } => {
            let (id, parents) = target_regions(design, target);
            let mut regions = Vec::new();
            for (start, parent_width) in parents {
                if let ExpressionKind::Constant(literal) = &offset.kind {
                    if let Ok(offset) = usize::try_from(&literal.value)
                        && offset
                            .checked_add(*width)
                            .is_some_and(|end| end <= parent_width)
                    {
                        regions.push((start + offset, *width));
                    }
                } else if *width <= parent_width {
                    regions
                        .extend((0..=parent_width - width).map(|offset| (start + offset, *width)));
                }
            }
            (id, regions)
        }
        AssignmentTarget::ArrayElement {
            array,
            index,
            dtype,
        } => {
            let (id, parents) = target_regions(design, array);
            let layout = design.data_type(*dtype).unpacked.as_ref().unwrap();
            let offsets = if let ExpressionKind::Constant(literal) = &index.kind {
                layout
                    .indices
                    .into_iter()
                    .enumerate()
                    .filter(|(_, declared)| {
                        usize::try_from(&literal.value)
                            .ok()
                            .is_some_and(|value| Some(value) == usize::try_from(*declared).ok())
                    })
                    .map(|(offset, _)| offset)
                    .collect::<Vec<_>>()
            } else {
                (0..layout.indices.len()).collect()
            };
            (
                id,
                parents
                    .into_iter()
                    .flat_map(|(start, _)| {
                        (&offsets).into_iter().map(move |offset| {
                            (start + offset * layout.element_width, layout.element_width)
                        })
                    })
                    .collect(),
            )
        }
    }
}
