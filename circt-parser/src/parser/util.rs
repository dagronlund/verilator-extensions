use crate::ast::CombOperator;

pub(super) fn comb_operator(name: &str) -> Option<CombOperator> {
    Some(match name {
        "comb.add" => CombOperator::Add,
        "comb.sub" => CombOperator::Sub,
        "comb.mul" => CombOperator::Mul,
        "comb.divs" => CombOperator::DivSigned,
        "comb.divu" => CombOperator::DivUnsigned,
        "comb.mods" => CombOperator::ModSigned,
        "comb.modu" => CombOperator::ModUnsigned,
        "comb.and" => CombOperator::And,
        "comb.or" => CombOperator::Or,
        "comb.xor" => CombOperator::Xor,
        "comb.shl" => CombOperator::ShiftLeft,
        "comb.shrs" => CombOperator::ShiftRightSigned,
        "comb.shru" => CombOperator::ShiftRightUnsigned,
        _ => return None,
    })
}

pub(super) fn supported_operation(name: &str) -> bool {
    match name {
        "hw.constant"
        | "hw.output"
        | "hw.instance"
        | "hw.array_get"
        | "hw.array_inject"
        | "comb.icmp"
        | "comb.mux"
        | "comb.concat"
        | "comb.replicate"
        | "comb.extract"
        | "seq.to_clock"
        | "seq.firreg"
        | "seq.compreg"
        | "verif.clocked_assert"
        | "verif.clocked_assume"
        | "verif.clocked_cover" => true,
        _ => comb_operator(name).is_some(),
    }
}
