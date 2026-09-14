use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use verilator_parser::{
    ast::{
        AccessMode, AssignmentKind, AssignmentTarget, BinaryOperator, DataType, DataTypeId,
        DataTypeKind, Design, Direction, Domain, Edge, EnumVariant, Expression, ExpressionKind,
        Literal, SignalDomain, SourceInfo, Statement, StatementKind, Variable, VariableId,
        VariableKind, range::Range,
    },
    document::AstDocument,
};

use crate::{
    GenerateOptions, generate_project, indexed_field_name, relative_path, snake_identifier,
};

#[test]
fn sanitizes_and_disambiguates_rust_identifiers() {
    assert_eq!(snake_identifier("TOP.packet-data[0]"), "top_packet_data_0");
    assert_eq!(snake_identifier("type"), "type_");
    assert_eq!(snake_identifier("12bad"), "signal_12bad");
}

#[test]
fn recognizes_only_trailing_verilator_numeric_suffixes() {
    assert_eq!(
        indexed_field_name("pending_write_v123"),
        Some(("pending_write", 123))
    );
    assert_eq!(indexed_field_name("pending_v_write"), None);
    assert_eq!(indexed_field_name("pending_write_v"), None);
    assert_eq!(indexed_field_name("pending_write_v1_extra"), None);
}

#[test]
fn computes_relative_runtime_paths() {
    assert_eq!(
        relative_path(
            PathBuf::from("/workspace/output/model").as_path(),
            PathBuf::from("/workspace/verilator-rust/verilator-rust-runtime").as_path(),
        )
        .unwrap(),
        PathBuf::from("../../verilator-rust/verilator-rust-runtime")
    );
}

#[test]
fn generates_a_counter_project_with_relative_runtime() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let ast = root.join("tests/counter/build/ast.json");
    if !ast.exists() {
        return;
    }
    let document = AstDocument::from_path(ast).unwrap();
    let design = Design::try_from(&document).unwrap();
    let output = std::env::temp_dir().join(format!("verilator-rust-test-{}", std::process::id()));
    if output.exists() {
        fs::remove_dir_all(&output).unwrap();
    }
    generate_project(
        &design,
        &output,
        &GenerateOptions {
            crate_name: "generated-counter".to_string(),
            clock: Domain::new("clk", Edge::Positive),
            reset: None,
        },
    )
    .unwrap();
    let manifest = fs::read_to_string(output.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("verilator-rust-runtime = { path = \".."));
    assert!(!manifest.contains("path = \"/"));
    let library = fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(library.contains("pub use input::Inputs;"));
    assert!(library.contains("pub use output::Outputs;"));
    assert!(library.contains("pub use state::State;"));
    assert!(library.contains("pub use types::*;"));
    assert!(library.contains("struct Env {"));
    assert!(library.contains("values: Env,"));
    assert!(library.contains("::from_usize("));
    assert!(library.contains("let temp0 = "));
    assert!(!library.contains("let value_"));
    assert!(!library.contains(".clone()"));
    assert!(!library.contains(".resize::<"));
    assert!(!library.contains("Value"));
    assert!(
        fs::read_to_string(output.join("src/input.rs"))
            .unwrap()
            .contains("pub struct Inputs")
    );
    assert!(
        fs::read_to_string(output.join("src/output.rs"))
            .unwrap()
            .contains("pub struct Outputs")
    );
    assert!(
        fs::read_to_string(output.join("src/state.rs"))
            .unwrap()
            .contains("pub struct State")
    );
    assert!(output.join("src/types.rs").is_file());
    fs::create_dir(output.join("tests")).unwrap();
    fs::write(
        output.join("tests/smoke.rs"),
        r#"
use generated_counter::{Inputs, Model};
use verilator_rust_runtime::Bits;

#[test]
fn reset_increment_and_hold() {
    let mut model = Model::new();
    let mut inputs = Inputs { reset_n: Bits::from_bool(false), enable: Bits::from_bool(true) };
    assert_eq!(model.tick(&inputs).outputs.count.to_u128(), 0);
    inputs.reset_n = Bits::from_bool(true);
    assert_eq!(model.tick(&inputs).outputs.count.to_u128(), 1);
    assert_eq!(model.tick(&inputs).outputs.count.to_u128(), 2);
    inputs.enable = Bits::from_bool(false);
    assert_eq!(model.tick(&inputs).outputs.count.to_u128(), 2);
}
"#,
    )
    .unwrap();
    let status = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&output)
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn generates_settling_loop_for_unorderable_combinational_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let fixture = root.join("tests/combinational_loops");
    let status = Command::new("ninja")
        .arg("-C")
        .arg(&fixture)
        .status()
        .unwrap();
    assert!(status.success());

    let document = AstDocument::from_path(fixture.join("build/ast.json")).unwrap();
    let design = Design::try_from(&document).unwrap();
    let output = std::env::temp_dir().join(format!(
        "verilator-rust-combinational-loops-test-{}",
        std::process::id()
    ));
    if output.exists() {
        fs::remove_dir_all(&output).unwrap();
    }
    generate_project(
        &design,
        &output,
        &GenerateOptions {
            crate_name: "generated-combinational-loops".to_string(),
            clock: Domain::new("clk", Edge::Positive),
            reset: None,
        },
    )
    .unwrap();
    let library = fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(library.contains("fn run_combinational_pass(env: &mut Env)"));
    assert!(library.contains("combinational logic did not reach a fixed point"));
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn generates_every_cached_fixture() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("..");
    let output_root = manifest_dir.join("build");
    for fixture in [
        "case_statements",
        "counter_free",
        "counter_ones",
        "data_types",
        "fifo_stage",
        "fifo_stage_bypass",
        "fifo_stage_fail_assert",
        "fifo_stage_fail_free",
        "gecko_core",
        "multidim_arrays",
        "package_properties",
        "packet_switch",
        "public_submodules",
        "signed_operations",
        "wire_ports",
    ] {
        let ast = root.join("tests").join(fixture).join("build/ast.json");
        if !ast.exists() {
            continue;
        }
        let document = AstDocument::from_path(ast).unwrap();
        let design = Design::try_from(&document).unwrap();
        let output = output_root.join(fixture);
        let crate_name = format!("generated-{fixture}");
        replace_generated_project(
            &design,
            &output,
            &GenerateOptions {
                crate_name: crate_name.clone(),
                clock: Domain::new("clk", Edge::Positive),
                reset: None,
            },
        )
        .unwrap_or_else(|error| panic!("failed to generate {fixture}: {error}"));

        let integration_tests = output.join("tests");
        if integration_tests.exists() {
            fs::remove_dir_all(&integration_tests).unwrap();
        }
        if fixture != "gecko_core" {
            let library = output.join("src/lib.rs");
            let mut source = fs::read_to_string(&library).unwrap();
            source.push_str("\n#[cfg(test)]\nmod tests;\n");
            fs::write(library, source).unwrap();
            fs::write(
                output.join("src/tests.rs"),
                r#"use super::{Inputs, Model};

#[test]
fn eval_and_tick_once() {
    let mut model = Model::new();
    let inputs = Inputs::default();
    let _ = model.eval(&inputs);
    let _ = model.tick(&inputs);
}
"#,
            )
            .unwrap();
        }

        let status = Command::new("cargo")
            .args([
                if fixture == "gecko_core" {
                    "check"
                } else {
                    "test"
                },
                "--quiet",
            ])
            .current_dir(&output)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "generated fixture {fixture} did not pass Cargo validation"
        );
        if fixture == "case_statements" {
            let source = fs::read_to_string(output.join("src/lib.rs")).unwrap();
            assert!(source.contains(".select::<1, bool>(2)"));
        }
        if fixture == "multidim_arrays" {
            let library = fs::read_to_string(output.join("src/lib.rs")).unwrap();
            assert!(library.contains("array_offset(env.v"));
            assert!(library.contains(".to_usize(),"));
            let source = fs::read_to_string(output.join("src/types.rs")).unwrap();
            assert!(source.contains("pub fn get(&self, index: usize)"));
            assert!(source.contains("pub fn set(&mut self, index: usize,"));
        }
        if fixture == "gecko_core" {
            let source = fs::read_to_string(output.join("src/state.rs")).unwrap();
            assert!(!source.contains("_vdly"));
        }
    }
}

fn replace_generated_project(
    design: &Design,
    output: &Path,
    options: &GenerateOptions,
) -> Result<(), crate::GenerateError> {
    let name = output.file_name().unwrap().to_string_lossy();
    let staging = output.with_file_name(format!(".{name}-staging"));
    if staging.exists() {
        fs::remove_dir_all(&staging).unwrap();
    }
    generate_project(design, &staging, options)?;
    if !output.exists() {
        fs::rename(staging, output).unwrap();
        return Ok(());
    }

    let source = output.join("src");
    if source.exists() {
        fs::remove_dir_all(source).unwrap();
    }
    for file in ["Cargo.toml", "Cargo.lock"] {
        let path = output.join(file);
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }
    fs::rename(staging.join("Cargo.toml"), output.join("Cargo.toml")).unwrap();
    fs::rename(staging.join("src"), output.join("src")).unwrap();
    fs::remove_dir(staging).unwrap();
    Ok(())
}

#[test]
fn refuses_to_overwrite_a_nonempty_directory() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let ast = root.join("tests/counter/build/ast.json");
    if !ast.exists() {
        return;
    }
    let document = AstDocument::from_path(ast).unwrap();
    let design = Design::try_from(&document).unwrap();
    let output =
        std::env::temp_dir().join(format!("verilator-rust-nonempty-{}", std::process::id()));
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("keep"), "user data").unwrap();
    let error = generate_project(
        &design,
        &output,
        &GenerateOptions {
            crate_name: "generated-counter".to_string(),
            clock: Domain::new("clk", Edge::Positive),
            reset: None,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("is not empty"));
    assert_eq!(
        fs::read_to_string(output.join("keep")).unwrap(),
        "user data"
    );
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn unlowered_nonblocking_assignments_preserve_scheduling() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/nba_semantics");
    let status = Command::new("ninja")
        .arg("-C")
        .arg(&directory)
        .arg("build/.verilated")
        .status()
        .unwrap();
    assert!(status.success());
    let document = AstDocument::from_path(directory.join("build/ast.json")).unwrap();
    let design = Design::try_from(&document).unwrap();
    let output =
        std::env::temp_dir().join(format!("verilator-rust-nba-test-{}", std::process::id()));
    replace_generated_project(
        &design,
        &output,
        &GenerateOptions {
            crate_name: "generated-nba".into(),
            clock: Domain::new("clk", Edge::Positive),
            reset: None,
        },
    )
    .unwrap();
    fs::create_dir_all(output.join("tests")).unwrap();
    fs::copy(
        directory.join("expected.txt"),
        output.join("tests/expected.txt"),
    )
    .unwrap();
    fs::write(output.join("tests/semantics.rs"), r#"
use generated_nba::{Inputs, Model};
use verilator_rust_runtime::Bits;

#[test]
fn preserves_nonblocking_scheduling() {
    assert_eq!(include_str!("expected.txt").lines().count(), 32);
    let mut model = Model::new();
    for (step, line) in include_str!("expected.txt").lines().enumerate() {
        let inputs = Inputs { reset_n: Bits::from_bool(step != 0 && step != 17),
            enable: Bits::from_bool(step % 4 != 2), index: Bits::from_bool(step & 1 != 0), data: Bits::from_u8(((step * 19) & 255) as u8) };
        let evaluation = model.tick(&inputs);
        let actual = evaluation.outputs;
        let expected: Vec<u8> = line.split_whitespace().map(|word| word.parse().unwrap()).collect();
        assert_eq!([actual.a.to_u128() as u8, actual.b.to_u128() as u8, actual.pipeline.to_u128() as u8, actual.packed_value.to_u128() as u8, actual.temp_result.to_u128() as u8,
            actual.mem0.to_u128() as u8, actual.mem1.to_u128() as u8, actual.blocking_count.to_u128() as u8, actual.lane0.to_u128() as u8, actual.lane1.to_u128() as u8].as_slice(), expected.as_slice(), "step {step}");
        assert!(evaluation.assertions.assert_pipeline, "pipeline assertion, step {step}");
        assert!(evaluation.assertions.assert_sampled_blocking, "sampled assertion, step {step}");
        assert_eq!(model.eval(&inputs).outputs, actual);
    }
}
"#).unwrap();
    let status = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&output)
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn generated_storage_boundaries_preserve_wide_state_and_nonblocking_reads() {
    let source = SourceInfo {
        node_type: "storage-test".into(),
        address: None,
        location: None,
    };
    let mut design = Design {
        source: source.clone(),
        data_types: Vec::new(),
        variables: Vec::new(),
        sensitivity_domains: vec![SignalDomain {
            variable: VariableId(0),
            domain: Domain::new("clk", Edge::Positive),
        }],
        initial: Vec::new(),
        combinational: Vec::new(),
        sequential: Vec::new(),
    };
    let widths = [1, 8, 9, 16, 17, 32, 33, 64, 65, 128, 129, 257];
    let mut smoke = String::from(
        "use generated_storage::{Inputs, Model};\nuse verilator_rust_runtime::Bits;\n#[test]\nfn wraps_and_delays() {\nlet mut model = Model::new();\nlet mut inputs = Inputs::default();\n",
    );
    let mut checks = String::new();
    let mut ones = String::new();
    let mut wrapped = String::new();
    for width in widths {
        let dtype = DataTypeId(design.data_types.len());
        let range = Range {
            left: width as isize - 1,
            right: 0,
        };
        design.data_types.push(DataType {
            source: source.clone(),
            name: Some(format!("word_{width}")),
            width,
            indices: range,
            signed: false,
            unpacked: None,
            kind: DataTypeKind::Basic { packed: range },
        });
        let mut enumeration = design.data_types[dtype.0].clone();
        enumeration.name = Some(format!("choice_{width}"));
        enumeration.kind = DataTypeKind::Enum {
            base: dtype,
            variants: vec![EnumVariant {
                source: source.clone(),
                name: "ONE".into(),
                value: Literal {
                    spelling: "1".into(),
                    value: 1u8.into(),
                },
            }],
        };
        design.data_types.push(enumeration);
        let storage = super::util::storage_type(width);
        smoke.push_str(&format!("let word: generated_storage::Word{width} = Bits::<{width}, {storage}>::from_u8(1);\nassert_eq!(word.to_u128(), 1);\n"));
        let call = if width > 128 { "()" } else { "" };
        smoke.push_str(&format!(
            "assert_eq!(generated_storage::Choice{width}::ONE{call}.0.to_u128(), 1);\n"
        ));
        if width == 1 {
            design.variables.push(Variable {
                source: source.clone(),
                name: "clk".into(),
                original_name: None,
                dtype,
                direction: Direction::Input,
                kind: VariableKind::Port,
                top_level: true,
                internal: false,
                sampled_value: None,
                property: None,
            });
        }
        let input = VariableId(design.variables.len());
        let count = VariableId(input.0 + 1);
        let delayed = VariableId(input.0 + 2);
        for (name, direction) in [
            (format!("data_w{width}"), Direction::Input),
            (format!("count_w{width}"), Direction::Output),
            (format!("delayed_w{width}"), Direction::Output),
        ] {
            design.variables.push(Variable {
                source: source.clone(),
                name,
                original_name: None,
                dtype,
                direction,
                kind: VariableKind::Port,
                top_level: true,
                internal: false,
                sampled_value: None,
                property: None,
            });
        }
        let read = |variable| Expression {
            source: source.clone(),
            dtype,
            kind: ExpressionKind::Variable {
                variable,
                access: AccessMode::Read,
            },
        };
        for (variable, value) in [
            (
                count,
                Expression {
                    source: source.clone(),
                    dtype,
                    kind: ExpressionKind::Binary {
                        operator: BinaryOperator::Add,
                        lhs: Box::new(read(count)),
                        rhs: Box::new(read(input)),
                    },
                },
            ),
            (delayed, read(count)),
        ] {
            design.sequential.push(Statement {
                source: source.clone(),
                kind: StatementKind::Assignment {
                    kind: AssignmentKind::Nonblocking,
                    target: AssignmentTarget::Variable {
                        variable,
                        access: AccessMode::Write,
                    },
                    value,
                },
            });
        }
        let maximum = format!(
            "Bits::<{width}, {storage}>::from_words(&[u64::MAX; {}])",
            width.div_ceil(64)
        );
        let one = "Bits::from_u8(1)";
        let zero = "Bits::zero()";
        smoke.push_str(&format!("inputs.data_w{width} = {maximum};\n"));
        checks.push_str(&format!("assert_eq!(first.outputs.count_w{width}, {maximum});\nassert_eq!(first.outputs.delayed_w{width}, {zero});\n"));
        ones.push_str(&format!("inputs.data_w{width} = {one};\n"));
        wrapped.push_str(&format!("assert_eq!(second.outputs.count_w{width}, {zero});\nassert_eq!(second.outputs.delayed_w{width}, {maximum});\nassert_eq!(model.state().count_w{width}, {zero});\nassert_eq!(model.state().delayed_w{width}, {maximum});\n"));
    }
    smoke.push_str("let first = model.tick(&inputs);\nassert_eq!(model.eval(&inputs), first);\n");
    smoke.push_str(&checks);
    smoke.push_str(&ones);
    smoke.push_str("let second = model.tick(&inputs);\nassert_eq!(model.eval(&inputs), second);\n");
    smoke.push_str(&wrapped);
    smoke.push_str(&checks); // Previously returned wide values must remain unchanged.
    smoke.push_str("}\n");
    let output =
        std::env::temp_dir().join(format!("verilator-rust-storage-{}", std::process::id()));
    replace_generated_project(
        &design,
        &output,
        &GenerateOptions {
            crate_name: "generated-storage".into(),
            clock: Domain::new("clk", Edge::Positive),
            reset: None,
        },
    )
    .unwrap();
    let library = fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(!library.contains(".clone()"));
    for (width, storage) in [
        (1, "bool"),
        (8, "u8"),
        (9, "u16"),
        (17, "u32"),
        (33, "u64"),
        (65, "u128"),
        (129, "ruint::Uint<129, { ruint::nlimbs(129) }>"),
        (257, "ruint::Uint<257, { ruint::nlimbs(257) }>"),
    ] {
        assert!(library.contains(&format!("Bits<{width}, {storage}>")));
    }
    for (file, field) in [
        ("input.rs", "data"),
        ("output.rs", "count"),
        ("state.rs", "count"),
    ] {
        let source = fs::read_to_string(output.join("src").join(file)).unwrap();
        for width in widths {
            let parameters = super::util::bits_parameters(width);
            assert!(source.contains(&format!("pub {field}_w{width}: Bits<{parameters}>")));
        }
    }
    fs::create_dir_all(output.join("tests")).unwrap();
    fs::write(output.join("tests/storage.rs"), smoke).unwrap();
    let status = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(&output)
        .status()
        .unwrap();
    assert!(status.success());
    fs::remove_dir_all(output).unwrap();
}
