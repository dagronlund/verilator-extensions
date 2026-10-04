use std::{fs, path::PathBuf};

use bytes::Bytes;
use parser_circt::{
    ast::{
        AttributeValue, ClockEdge, CombOperator, ComparisonPredicate, ConstantValue, File,
        HwModule, Item, OperationKind, PortDirection, PropertyKind, ResetKind, Signedness, Type,
        Visibility,
    },
    lexer::{
        Lexer,
        token::{LexerToken, LexerTokenKind},
    },
    parser::{Parser, error::ParserError, parse},
    writer::{Writer, WriterNode, error::WriterError, write},
};

fn hw_module(source: &str) -> HwModule {
    let file = parse(7, source).unwrap();
    assert_eq!(file.items.len(), 1);
    match file.items.into_iter().next().unwrap() {
        Item::HwModule(module) => module,
        other => panic!("expected HW module, got {other:?}"),
    }
}

fn integer(width: u32) -> Type {
    Type::Integer {
        width,
        signedness: Signedness::Signless,
    }
}

#[test]
fn module_hierarchy_ports_and_locations() {
    let source = r#"builtin.module @design attributes {version = 1 : i32} {
        module {
            hw.module private @"some-module"(in %clk: !seq.clock,
                in %a: si4 {test.flag}, out "result": ui8 loc("port.sv":2:3),
                inout %wire: !hw.inout<i1>) attributes {comment = "hello"} {
                hw.output %a: si4
            } loc(fused["top", "source.sv":1:2])
        }
    } loc(unknown)"#;
    let file = parse(7, source).unwrap();
    let Item::Module(outer) = &file.items[0] else {
        panic!()
    };
    assert_eq!(outer.name.as_ref().unwrap().spelling.as_ref(), b"@design");
    assert_eq!(outer.position.file_id, 7);
    assert_eq!(outer.position.range(), 0..source.len());
    assert_eq!(outer.attributes.len(), 1);
    let Item::Module(inner) = &outer.items[0] else {
        panic!()
    };
    let Item::HwModule(module) = &inner.items[0] else {
        panic!()
    };
    assert_eq!(module.visibility, Visibility::Private);
    assert_eq!(module.name.spelling.as_ref(), br#"@"some-module""#);
    assert_eq!(module.ports[0].ty, Type::Clock);
    assert_eq!(module.ports[1].attributes[0].value, AttributeValue::Unit);
    assert_eq!(module.ports[2].direction, PortDirection::Output);
    assert_eq!(
        module.ports[2].location.as_ref().unwrap().spelling.as_ref(),
        br#"loc("port.sv":2:3)"#
    );
    assert_eq!(module.ports[3].direction, PortDirection::InOut);
    assert_eq!(module.ports[3].ty, Type::InOut(Box::new(integer(1))));
    assert_eq!(module.attributes.len(), 1);
    assert_eq!(
        outer.location.as_ref().unwrap().spelling.as_ref(),
        b"loc(unknown)"
    );
}

#[test]
fn nested_hw_types_and_compact_array_separators() {
    let module = hw_module(
        "hw.module @types(in %a: !hw.array<2xarray<3xi4>>, in %b: !hw.array<2 x i8>, in %c: !hw.struct<a: i4, b: !hw.array<3xi1>>, in %d: !word) { hw.output }",
    );
    assert_eq!(
        module.ports[0].ty,
        Type::Array {
            size: 2,
            element: Box::new(Type::Array {
                size: 3,
                element: Box::new(integer(4))
            }),
        }
    );
    assert_eq!(
        module.ports[1].ty,
        Type::Array {
            size: 2,
            element: Box::new(integer(8))
        }
    );
    let Type::Struct(fields) = &module.ports[2].ty else {
        panic!()
    };
    assert_eq!(
        fields[1].ty,
        Type::Array {
            size: 3,
            element: Box::new(integer(1))
        }
    );
    let Type::Alias(name) = &module.ports[3].ty else {
        panic!()
    };
    assert_eq!(name.spelling.as_ref(), b"!word");
}

#[test]
fn constants_preserve_arbitrary_precision_and_spelling() {
    let huge = "9".repeat(300);
    let module = hw_module(&format!(
        "hw.module @constants() {{
        %t = hw.constant true %f = hw.constant false
        %n = hw.constant -{huge} : i1024
        %h = hw.constant 0xFF : i8
        hw.output
    }}"
    ));
    let body = module.body.unwrap();
    assert_eq!(
        body[0].kind,
        OperationKind::Constant {
            value: ConstantValue::Boolean(true),
            ty: integer(1)
        }
    );
    assert_eq!(
        body[1].kind,
        OperationKind::Constant {
            value: ConstantValue::Boolean(false),
            ty: integer(1)
        }
    );
    let OperationKind::Constant {
        value: ConstantValue::Integer(value),
        ty,
    } = &body[2].kind
    else {
        panic!()
    };
    assert_eq!(value.spelling.as_ref(), format!("-{huge}").as_bytes());
    assert_eq!(*ty, integer(1024));
    let OperationKind::Constant {
        value: ConstantValue::Integer(value),
        ..
    } = &body[3].kind
    else {
        panic!()
    };
    assert_eq!(value.spelling.as_ref(), b"0xFF");
}

#[test]
fn grouped_instance_results_and_forward_ssa_references() {
    let module = hw_module(
        r#"hw.module @parent(in %clk: i1, out y: i4) {
        %pair:2 = hw.instance "child\22" @child(clk: %clk: i1, d: %later: i4) -> (a: i4, b: i4) {sv.namehint = "child"}
        %later = comb.add bin %pair#0, %pair#1: i4
        hw.output %later: i4
    }"#,
    );
    let body = module.body.unwrap();
    assert_eq!(body[0].results[0].count, 2);
    assert_eq!(body[0].attributes.len(), 1);
    let OperationKind::Instance {
        name,
        module,
        inputs,
        outputs,
    } = &body[0].kind
    else {
        panic!()
    };
    assert_eq!(name.spelling.as_ref(), br#""child\22""#);
    assert_eq!(module.spelling.as_ref(), b"@child");
    assert_eq!(inputs[1].value.name.spelling.as_ref(), b"%later");
    assert_eq!(outputs.len(), 2);
    let OperationKind::Comb {
        operator,
        two_state,
        operands,
        ..
    } = &body[1].kind
    else {
        panic!()
    };
    assert_eq!(*operator, CombOperator::Add);
    assert!(*two_state);
    assert_eq!(operands[0].result_index, Some(0));
    assert_eq!(operands[1].result_index, Some(1));
}

#[test]
fn arrays_muxes_concat_extract_and_attributes() {
    let source = r#"hw.module @array(in %idx: i1, in %data: i4) {
        %get = hw.array_get %memory[%idx] : !hw.array<2xi4>, i1
        %update = hw.array_inject %memory[%idx], %data : !hw.array<2xi4>, i1
        %next = comb.mux bin %idx, %update, %memory : !hw.array<2xi4>
        %cat = comb.concat %idx, %data {sv.namehint = "cat", values = [true, -2: i4, {flag}]} : i1, i4
        %bit = comb.extract %cat from 2 {sv.namehint = "bit"} : (i5) -> i1 loc("array.sv":4:5)
        hw.output
    }"#;
    let body = hw_module(source).body.unwrap();
    let OperationKind::ArrayGet {
        array_type,
        index_type,
        ..
    } = &body[0].kind
    else {
        panic!()
    };
    assert_eq!(
        *array_type,
        Type::Array {
            size: 2,
            element: Box::new(integer(4))
        }
    );
    assert_eq!(*index_type, integer(1));
    let OperationKind::ArrayInject { value, .. } = &body[1].kind else {
        panic!()
    };
    assert_eq!(value.name.spelling.as_ref(), b"%data");
    let OperationKind::Mux { two_state, .. } = &body[2].kind else {
        panic!()
    };
    assert!(*two_state);
    let OperationKind::Concat { types, .. } = &body[3].kind else {
        panic!()
    };
    assert_eq!(*types, vec![integer(1), integer(4)]);
    let AttributeValue::Array(values) = &body[3].attributes[1].value else {
        panic!()
    };
    assert_eq!(values.len(), 3);
    let OperationKind::Extract {
        offset,
        input_type,
        result_type,
        ..
    } = &body[4].kind
    else {
        panic!()
    };
    assert_eq!(*offset, 2);
    assert_eq!(*input_type, integer(5));
    assert_eq!(*result_type, integer(1));
    assert!(source[body[4].position.range()].ends_with("loc(\"array.sv\":4:5)"));
}

#[test]
fn registers_resets_presets_and_property_labels() {
    let body = hw_module(
        r#"hw.module @regs(in %clk: i1) {
        %clock = seq.to_clock %clk
        %r = seq.firreg %next clock %clock sym @reg reset async %rst, %zero preset 0xF : i4
        %s = seq.firreg %next clock %clock reset sync %rst, %zero : i4
        %c = seq.compreg "previous" %r, %clock reset %rst, %zero : i4
        verif.clocked_assert %ok if %en, negedge %clk label "check" {test.flag} : i1
        verif.clocked_assume %ok, posedge %clk : i1
        verif.clocked_cover %ok, posedge %clk label "" : i1
        hw.output
    }"#,
    )
    .body
    .unwrap();
    let OperationKind::FirReg {
        reset,
        preset,
        symbol,
        ..
    } = &body[1].kind
    else {
        panic!()
    };
    assert_eq!(reset.as_ref().unwrap().kind, ResetKind::Async);
    assert_eq!(preset.as_ref().unwrap().spelling.as_ref(), b"0xF");
    assert_eq!(symbol.as_ref().unwrap().spelling.as_ref(), b"@reg");
    let OperationKind::FirReg { reset, preset, .. } = &body[2].kind else {
        panic!()
    };
    assert_eq!(reset.as_ref().unwrap().kind, ResetKind::Sync);
    assert!(preset.is_none());
    let OperationKind::CompReg { name, reset, .. } = &body[3].kind else {
        panic!()
    };
    assert!(name.is_some());
    assert_eq!(reset.as_ref().unwrap().kind, ResetKind::Sync);
    let OperationKind::ClockedProperty {
        kind,
        enable,
        edge,
        label,
        ..
    } = &body[4].kind
    else {
        panic!()
    };
    assert_eq!(*kind, PropertyKind::Assert);
    assert!(enable.is_some());
    assert_eq!(*edge, ClockEdge::Negative);
    assert_eq!(label.as_ref().unwrap().spelling.as_ref(), br#""check""#);
    assert_eq!(body[4].attributes[0].value, AttributeValue::Unit);
    let OperationKind::ClockedProperty {
        kind,
        enable,
        label,
        ..
    } = &body[5].kind
    else {
        panic!()
    };
    assert_eq!(*kind, PropertyKind::Assume);
    assert!(enable.is_none());
    assert!(label.is_none());
    let OperationKind::ClockedProperty { kind, label, .. } = &body[6].kind else {
        panic!()
    };
    assert_eq!(*kind, PropertyKind::Cover);
    assert_eq!(label.as_ref().unwrap().spelling.as_ref(), br#""""#);
}

#[test]
fn comparisons_and_binary_operations_are_typed() {
    for (spelling, predicate) in [
        ("eq", ComparisonPredicate::Equal),
        ("ne", ComparisonPredicate::NotEqual),
        ("slt", ComparisonPredicate::SignedLess),
        ("sle", ComparisonPredicate::SignedLessEqual),
        ("sgt", ComparisonPredicate::SignedGreater),
        ("sge", ComparisonPredicate::SignedGreaterEqual),
        ("ult", ComparisonPredicate::UnsignedLess),
        ("ule", ComparisonPredicate::UnsignedLessEqual),
        ("ugt", ComparisonPredicate::UnsignedGreater),
        ("uge", ComparisonPredicate::UnsignedGreaterEqual),
    ] {
        let body = hw_module(&format!(
            "hw.module @cmp() {{ %r = comb.icmp bin {spelling} %a, %b: i4 hw.output }}"
        ))
        .body
        .unwrap();
        let OperationKind::Compare {
            predicate: actual,
            two_state,
            ty,
            ..
        } = &body[0].kind
        else {
            panic!()
        };
        assert_eq!(*actual, predicate);
        assert!(*two_state);
        assert_eq!(*ty, integer(4));
    }
    let body = hw_module("hw.module @div() { %r = comb.divs bin %a, %b: i4 hw.output }")
        .body
        .unwrap();
    let OperationKind::Comb { operator, .. } = &body[0].kind else {
        panic!()
    };
    assert_eq!(*operator, CombOperator::DivSigned);
}

#[test]
fn external_modules_empty_output_and_empty_files() {
    assert!(parse(0, " // empty\n ").unwrap().items.is_empty());
    let file = parse(0, "hw.module.extern private @external(in %a: i4, out result: i4) hw.module @empty() { hw.output }").unwrap();
    assert_eq!(file.items.len(), 2);
    let Item::HwModule(module) = &file.items[0] else {
        panic!()
    };
    assert!(module.body.is_none());
    let Item::HwModule(module) = &file.items[1] else {
        panic!()
    };
    assert_eq!(
        module.body.as_ref().unwrap()[0].kind,
        OperationKind::Output {
            values: vec![],
            types: vec![]
        }
    );
}

#[test]
fn grammar_is_independent_of_comments_and_line_breaks_and_ast_is_owned() {
    let file = {
        let source = String::from(
            "module\n{ hw.module @test(in\n%a : i4) { %r = comb.add // comment\n bin %a,\n %a : i4 hw.output } }",
        );
        Parser::new(1, &source).unwrap().parse().unwrap()
    };
    let Item::Module(module) = &file.items[0] else {
        panic!()
    };
    let Item::HwModule(module) = &module.items[0] else {
        panic!()
    };
    assert_eq!(module.name.spelling.as_ref(), b"@test");
    assert_eq!(module.body.as_ref().unwrap().len(), 2);
}

#[test]
fn malformed_and_unsupported_assembly_is_rejected() {
    for source in [
        "module {",
        "hw.module @m(in %a i4) {}",
        "hw.module @m(in %a: i0) {}",
        "hw.module @m(in %a: i4294967296) {}",
        "hw.module @m(in %a: !hw.array<2i4>) {}",
        "hw.module @m(in %a: !hw.array<2xi4>) {} garbage",
        "hw.module @m(in %a: !hw.array<18446744073709551616xi4>) {}",
        "hw.module @m() { %r = hw.constant 1 }",
        "hw.module @m() { %r = hw.constant true : i1 }",
        "hw.module @m() { %r = hw.constant 1 {test.flag} : i4 }",
        "hw.module @m() { hw.constant true }",
        "hw.module @m() { %r = hw.output }",
        "hw.module @m() { %r, %s = hw.constant true }",
        "hw.module @m() { %r:0 = hw.constant true }",
        "hw.module @m() { %r = comb.icmp invalid %a, %b : i4 }",
        "hw.module @m() { %r = comb.sub %a, %b, %c : i4 }",
        "hw.module @m() { %r = comb.add %a, : i4 }",
        "hw.module @m() { %r = comb.concat %a, %b : i4 }",
        "hw.module @m() { hw.output %a, %b : i4 }",
        "hw.module @m() { %r = hw.instance \"i\" @child() -> (a:i4, b:i4) }",
        "hw.module @m() { %r = comb.add %a#name, %b : i4 }",
        "hw.module @m() { %r = comb.extract %a from -1 : (i4) -> i1 }",
        "hw.module @m() { %r = seq.firreg %a clock %c reset %r, %v : i4 }",
        "hw.module @m() { %r = seq.firreg %a clock %c preset -1 : i4 }",
        "hw.module @m() { verif.clocked_assert %a, invalid %c : i1 }",
        "hw.module @m() { %r = unknown.op %a : i4 }",
        "hw.module @m() { %r = \"hw.constant\"() {value=1:i4} : () -> i4 }",
        "hw.module @m() {} loc(fused[unknown))",
    ] {
        let error = parse(9, source).expect_err(source);
        assert_eq!(error.position().file_id, 9, "{source}");
        assert!(error.position().index <= source.len(), "{source}");
    }
}

#[test]
fn errors_report_token_and_eof_positions() {
    let source = "module {\r\n hw.module @m() {\r\n  unknown.op\r\n }\r\n}";
    let error = parse(9, source).unwrap_err();
    assert_eq!(error.position().line, 3);
    assert_eq!(error.position().column, 3);
    let ParserError::Syntax { message, .. } = error else {
        panic!()
    };
    assert!(message.contains("unsupported operation `unknown.op`"));
    let error = parse(9, "module {\r\n").unwrap_err();
    assert_eq!(error.position().index, 10);
    assert_eq!(error.position().line, 2);
    assert_eq!(error.position().column, 1);
    assert_eq!(error.position().length, 0);
    let error = parse(9, "module { ` }").unwrap_err();
    let ParserError::Lexer(error) = error else {
        panic!()
    };
    assert_eq!(error.position.column, 10);
}

#[test]
fn parses_every_generated_formal_core_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests");
    let mut directories = vec![root.clone()];
    let mut paths = Vec::new();
    // Include gitignored build directories without following directory symlinks.
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                directories.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "opt-formal-core.mlir")
                && path.is_file()
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no opt-formal-core.mlir files under {}; run ./test-circt.sh to generate them",
        root.display()
    );
    let mut total_modules = 0;
    let mut total_operations = 0;
    for (file_id, path) in (&paths).into_iter().enumerate() {
        let source = fs::read_to_string(path).unwrap();
        let file =
            parse(file_id, &source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_round_trip(&file, file_id);
        let mut modules = Vec::new();
        collect_modules(&file.items, &mut modules);
        assert!(!modules.is_empty(), "{}: empty AST", path.display());
        let mut operations = 0;
        for module in &modules {
            assert_eq!(module.position.file_id, file_id);
            assert!(source[module.position.range()].starts_with("hw.module"));
            let body = module.body.as_ref().expect("defined HW module");
            operations += body.len();
            let outputs = (&module.ports)
                .into_iter()
                .filter(|port| port.direction == PortDirection::Output)
                .count();
            let OperationKind::Output { values, .. } =
                &body.last().expect("HW output terminator").kind
            else {
                panic!("{}: missing hw.output", path.display())
            };
            assert_eq!(
                values.len(),
                outputs,
                "{}: {} output ports",
                path.display(),
                String::from_utf8_lossy(&module.name.spelling)
            );
            for operation in body {
                assert_eq!(operation.position.file_id, file_id);
                assert!(source.get(operation.position.range()).is_some());
                for result in &operation.results {
                    assert_eq!(
                        source[result.name.position.range()].as_bytes(),
                        result.name.spelling.as_ref()
                    );
                }
            }
        }
        // Independently count operation names in the lexer stream: parsing must
        // retain every operation, rather than just accepting/skipping the file.
        let mut expected_modules = 0;
        let mut expected_operations = 0;
        let original_tokens = Lexer::new(file_id, &source)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .filter(|token| !token.kind.is_whitespace())
            .collect::<Vec<_>>();
        assert_tokens_equal(&original_tokens, &file.to_vec(file_id).unwrap());
        for token in &original_tokens {
            if token.kind != LexerTokenKind::Identifier {
                continue;
            }
            let name = String::from_utf8_lossy(&token.text);
            if name == "hw.module" {
                expected_modules += 1;
            } else if name.starts_with("hw.")
                || name.starts_with("comb.")
                || name.starts_with("seq.")
                || name.starts_with("verif.")
            {
                expected_operations += 1;
            }
        }
        assert_eq!(modules.len(), expected_modules, "{}", path.display());
        assert_eq!(operations, expected_operations, "{}", path.display());
        total_modules += modules.len();
        total_operations += operations;
        println!(
            "{}: {} modules, {operations} operations; token round trip verified",
            path.display(),
            modules.len()
        );
    }
    println!(
        "Round-tripped {} formal-core fixtures: {total_modules} HW modules, {total_operations} operations",
        paths.len()
    );
}

fn collect_modules<'a>(items: &'a [Item], modules: &mut Vec<&'a HwModule>) {
    for item in items {
        match item {
            Item::Module(module) => collect_modules(&module.items, modules),
            Item::HwModule(module) => modules.push(module),
        }
    }
}

fn assert_tokens_equal(expected: &[LexerToken], actual: &[LexerToken]) {
    assert_eq!(expected.len(), actual.len(), "token count changed");
    for (index, (expected, actual)) in expected.into_iter().zip(actual).enumerate() {
        assert_eq!(expected.kind, actual.kind, "token {index} kind changed");
        assert_eq!(expected.text, actual.text, "token {index} text changed");
    }
}

fn assert_round_trip(file: &File, file_id: usize) -> File {
    let output = write(file_id, file).unwrap();
    let mut lexer = Lexer::new(file_id, output.source());
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_syntax().unwrap() {
        tokens.push(token);
    }
    assert_tokens_equal(output.tokens(), &tokens);
    let reparsed = Parser::from_tokens(file_id, output.source(), output.tokens().to_vec())
        .and_then(Parser::parse)
        .unwrap_or_else(|error| {
            panic!(
                "written assembly did not parse: {error}\n{}",
                output.source()
            )
        });
    let second = write(file_id, &reparsed).unwrap();
    assert_tokens_equal(output.tokens(), second.tokens());
    reparsed
}

#[test]
fn writes_edited_ast_fields_instead_of_replaying_original_tokens() {
    let mut file = parse(
        0,
        r#"hw.module @before(in %a: i4, in %b: i4, out y: i4) {
        %true = hw.constant true
        %r = comb.add %a, %b {sv.namehint = "before"} : i4
        hw.output %r : i4
    }"#,
    )
    .unwrap();
    let Item::HwModule(module) = &mut file.items[0] else {
        panic!()
    };
    module.name.spelling = Bytes::from_static(b"@after");
    module.visibility = Visibility::Nested;
    let body = module.body.as_mut().unwrap();
    let OperationKind::Constant { value, .. } = &mut body[0].kind else {
        panic!()
    };
    *value = ConstantValue::Boolean(false);
    let OperationKind::Comb {
        operator,
        two_state,
        operands,
        ..
    } = &mut body[1].kind
    else {
        panic!()
    };
    *operator = CombOperator::Xor;
    *two_state = true;
    operands[0].name.spelling = Bytes::from_static(b"%b");
    let AttributeValue::String(value) = &mut body[1].attributes[0].value else {
        panic!()
    };
    value.spelling = Bytes::from_static(br#""after""#);
    let output = write(11, &file).unwrap();
    assert!(output.source().contains("hw.module nested @after"));
    assert!(output.source().contains("hw.constant false"));
    assert!(output.source().contains("comb.xor bin %b , %b"));
    assert!(output.source().contains(r#"sv.namehint = "after""#));
    assert_round_trip(&file, 11);
}

#[test]
fn retained_multiline_literals_and_locations_have_correct_generated_positions() {
    let source = "module @outer {\r\n hw.module @m() attributes {label = \"λ\\n\\22\"} {\r\n %n = hw.constant - // literal\r\n 42 : i64\r\n hw.output\r\n } loc(fused[\r\n \"λ.sv\":1:2, unknown])\r\n}";
    let file = parse(8, source).unwrap();
    assert_round_trip(&file, 12);
    let output = write(12, &file).unwrap();
    assert!(output.source().contains("- // literal\r\n 42"));
    assert!(
        output
            .source()
            .contains("loc(fused[\r\n \"λ.sv\":1:2, unknown])")
    );
    assert!(
        output
            .tokens()
            .into_iter()
            .all(|token| token.position.file_id == 12)
    );
}

#[test]
fn all_operator_and_predicate_variants_round_trip() {
    for operator in [
        "add", "sub", "mul", "divs", "divu", "mods", "modu", "and", "or", "xor", "shl", "shrs",
        "shru",
    ] {
        for binary in ["", "bin"] {
            let file = parse(
                0,
                &format!(
                    "hw.module @m() {{ %r = comb.{operator} {binary} %a, %b : i4 hw.output }}"
                ),
            )
            .unwrap();
            assert_round_trip(&file, 1);
        }
    }
    for predicate in [
        "eq", "ne", "slt", "sle", "sgt", "sge", "ult", "ule", "ugt", "uge", "ceq", "cne", "weq",
        "wne",
    ] {
        let file = parse(
            0,
            &format!("hw.module @m() {{ %r = comb.icmp {predicate} %a, %b : i4 hw.output }}"),
        )
        .unwrap();
        assert_round_trip(&file, 1);
    }
}

#[test]
fn attributes_and_empty_nodes_round_trip() {
    for source in [
        "",
        "module {}",
        "hw.module @m() {}",
        "hw.module.extern public @m() attributes {flag}",
        r#"module attributes {items = [], dict = {}, ty = !hw.struct<field: i4>, symbol = @"some-module", number = +12} {
            hw.module @m() {
                hw.instance "no_results" @empty() -> () {items = ["", false, {flag}, 0xAB]}
                %c = hw.constant 3 : i4 {sv.namehint = "constant"}
                %true = hw.constant true {test.flag}
                hw.output {flag} loc(unknown)
            }
        }"#,
        "hw.module @m(in %a: !hw.array<2xstruct<a: i4, b: !hw.inout<i1>>>) { hw.output }",
    ] {
        assert_round_trip(&parse(0, source).unwrap(), 2);
    }
}

#[test]
fn convenience_methods_and_appending_nodes_produce_matching_tokens() {
    let file = parse(
        0,
        "hw.module @a() { hw.output } hw.module @b() { hw.output }",
    )
    .unwrap();
    let output = write(5, &file).unwrap();
    assert_eq!(file.to_vec(5).unwrap(), output.tokens());
    assert_eq!(file.to_source().unwrap(), output.source());
    let mut writer = Writer::new(5);
    for item in &file.items {
        item.write(&mut writer).unwrap();
    }
    let reparsed = Parser::from_tokens(5, writer.source(), writer.tokens().to_vec())
        .unwrap()
        .parse()
        .unwrap();
    assert_round_trip(&reparsed, 5);
    let empty = parse(0, "").unwrap();
    assert!(empty.to_source().unwrap().is_empty());
    assert!(empty.to_vec(0).unwrap().is_empty());
}

#[test]
fn invalid_raw_spellings_and_mismatched_token_sources_return_errors() {
    let mut file = parse(0, "hw.module @m() {}").unwrap();
    let Item::HwModule(module) = &mut file.items[0] else {
        panic!()
    };
    module.name.spelling = Bytes::from_static(&[0xff]);
    match write(0, &file) {
        Err(WriterError::Utf8(_)) => {}
        _ => panic!("expected UTF-8 error"),
    }
    let Item::HwModule(module) = &mut file.items[0] else {
        panic!()
    };
    module.name.spelling = Bytes::from_static(b"@\"unfinished");
    match write(0, &file) {
        Err(WriterError::Lexer(_)) => {}
        _ => panic!("expected lexer error"),
    }

    let source = "module {}";
    let tokens = Lexer::new(1, source)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    // The token constructor accepts both full and syntax-only lexer streams.
    Parser::from_tokens(1, source, tokens.clone())
        .unwrap()
        .parse()
        .unwrap();
    assert!(Parser::from_tokens(2, source, tokens.clone()).is_err());
    assert!(Parser::from_tokens(1, "module { }", tokens.clone()).is_err());
    let mut invalid = tokens.clone();
    invalid[0].position.length = usize::MAX;
    assert!(Parser::from_tokens(1, source, invalid).is_err());
    let mut invalid = tokens;
    invalid[0].text = Bytes::from_static(b"another");
    assert!(Parser::from_tokens(1, source, invalid).is_err());
}

#[test]
fn rejects_edits_that_would_silently_lose_constant_types_or_reset_kinds() {
    let mut file = parse(0, "hw.module @m() { %t = hw.constant true hw.output }").unwrap();
    let Item::HwModule(module) = &mut file.items[0] else {
        panic!()
    };
    let OperationKind::Constant { ty, .. } = &mut module.body.as_mut().unwrap()[0].kind else {
        panic!()
    };
    *ty = Type::Integer {
        width: 4,
        signedness: Signedness::Signless,
    };
    match write(0, &file) {
        Err(WriterError::InvalidAst(_)) => {}
        _ => panic!("expected invalid boolean constant type error"),
    }

    let mut file = parse(
        0,
        "hw.module @m() { %r = seq.compreg %a, %c reset %rst, %zero : i4 hw.output }",
    )
    .unwrap();
    let Item::HwModule(module) = &mut file.items[0] else {
        panic!()
    };
    let OperationKind::CompReg { reset, .. } = &mut module.body.as_mut().unwrap()[0].kind else {
        panic!()
    };
    reset.as_mut().unwrap().kind = ResetKind::Async;
    match write(0, &file) {
        Err(WriterError::InvalidAst(_)) => {}
        _ => panic!("expected unsupported asynchronous compreg reset error"),
    }
}
