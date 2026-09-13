use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use verilator_parser::{
    ast::{Design, Domain, Edge},
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

#[test]
fn reset_increment_and_hold() {
    let mut model = Model::new();
    let mut inputs = Inputs { reset_n: false, enable: true };
    assert_eq!(model.tick(&inputs).outputs.count, 0);
    inputs.reset_n = true;
    assert_eq!(model.tick(&inputs).outputs.count, 1);
    assert_eq!(model.tick(&inputs).outputs.count, 2);
    inputs.enable = false;
    assert_eq!(model.tick(&inputs).outputs.count, 2);
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
            assert!(source.contains(".select::<1>(2)"));
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
            assert!(source.contains("_vdlyval_data: [bool; 59],"));
            assert!(!source.contains("_vdlyval_data_v0:"));
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
