use std::collections::BTreeSet;

use crate::ast::{
    AssignmentTarget, Expression, ExpressionKind, Statement, StatementKind, VariableId,
};

/// Trait for collecting variable accesses (reads and writes) from AST nodes.
/// Some AST nodes can contain other AST nodes, so this trait allows for
/// recursive collection of variable accesses. Additionally, some AST nodes
/// may only contains reads, but this trait allows for collecting both reads and
/// writes.
pub trait CollectAccesses {
    fn collect_accesses(&self, reads: &mut BTreeSet<VariableId>, writes: &mut BTreeSet<VariableId>);

    fn collect_reads(&self, reads: &mut BTreeSet<VariableId>) {
        self.collect_accesses(reads, &mut BTreeSet::new());
    }

    fn collect_writes(&self, writes: &mut BTreeSet<VariableId>) {
        self.collect_accesses(&mut BTreeSet::new(), writes);
    }
}

impl<T: CollectAccesses> CollectAccesses for &[T] {
    fn collect_accesses(
        &self,
        reads: &mut BTreeSet<VariableId>,
        writes: &mut BTreeSet<VariableId>,
    ) {
        for item in *self {
            item.collect_accesses(reads, writes);
        }
    }
}

impl CollectAccesses for Statement {
    fn collect_accesses(
        &self,
        reads: &mut BTreeSet<VariableId>,
        writes: &mut BTreeSet<VariableId>,
    ) {
        match &self.kind {
            StatementKind::Block { statements, .. } => {
                for statement in statements {
                    statement.collect_accesses(reads, writes);
                }
            }
            StatementKind::Assignment { target, value, .. } => {
                target.collect_accesses(reads, writes);
                value.collect_accesses(reads, writes);
            }
            StatementKind::If {
                condition,
                then_statements,
                else_statements,
            } => {
                condition.collect_accesses(reads, writes);
                for statement in then_statements.into_iter().chain(else_statements) {
                    statement.collect_accesses(reads, writes);
                }
            }
        }
    }
}

impl CollectAccesses for AssignmentTarget {
    fn collect_accesses(
        &self,
        reads: &mut BTreeSet<VariableId>,
        writes: &mut BTreeSet<VariableId>,
    ) {
        match self {
            AssignmentTarget::Variable { variable, .. } => {
                writes.insert(*variable);
            }
            AssignmentTarget::Select { target, offset, .. } => {
                let mut target_writes = BTreeSet::new();
                target.collect_accesses(reads, &mut target_writes);
                reads.extend(&target_writes);
                writes.extend(target_writes);
                offset.collect_accesses(reads, writes);
            }
            AssignmentTarget::ArrayElement { array, index, .. } => {
                let mut target_writes = BTreeSet::new();
                array.collect_accesses(reads, &mut target_writes);
                reads.extend(&target_writes);
                writes.extend(target_writes);
                index.collect_accesses(reads, writes);
            }
        }
    }
}

impl CollectAccesses for Expression {
    fn collect_accesses(
        &self,
        reads: &mut BTreeSet<VariableId>,
        _writes: &mut BTreeSet<VariableId>,
    ) {
        match &self.kind {
            ExpressionKind::Constant(_) => {}
            ExpressionKind::Variable { variable, .. } => {
                reads.insert(*variable);
            }
            ExpressionKind::Unary { operand, .. } => operand.collect_reads(reads),
            ExpressionKind::Binary { lhs, rhs, .. } => {
                lhs.collect_reads(reads);
                rhs.collect_reads(reads);
            }
            ExpressionKind::Conditional {
                condition,
                then_value,
                else_value,
            } => {
                condition.collect_reads(reads);
                then_value.collect_reads(reads);
                else_value.collect_reads(reads);
            }
            ExpressionKind::Replicate { source, .. } => source.collect_reads(reads),
            ExpressionKind::Select { value, offset, .. } => {
                value.collect_reads(reads);
                offset.collect_reads(reads);
            }
            ExpressionKind::ArraySelect { array, index } => {
                array.collect_reads(reads);
                index.collect_reads(reads);
            }
        }
    }
}
