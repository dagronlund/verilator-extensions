//! Explicit integration checks: cargo test -p circt-formal --test formal_checks -- --ignored
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use circt_formal::{
    convert::{ConversionOptions, NamedFsm},
    export::{self, AigerFormat, ExportOptions},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "circt-formal-solvers-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn options(reset: Option<&str>) -> ConversionOptions {
    ConversionOptions {
        top: None,
        clock: "clk".parse().unwrap(),
        reset: reset.map(|value| value.parse().unwrap()),
    }
}
fn fixture(name: &str) -> NamedFsm {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../circt-parser/res/{name}.hw.mlir")),
    )
    .unwrap();
    NamedFsm::from_source(0, &source, &options(Some("!reset_n"))).unwrap()
}
fn ric3(model: &NamedFsm, selection: ExportOptions, expected: &str) {
    let directory = Temp::new();
    let path = directory.0.join("model.aig");
    fs::write(
        &path,
        export::write(&model.fsm, AigerFormat::Binary, &selection).unwrap(),
    )
    .unwrap();
    let output = Command::new("ric3")
        .arg("check")
        .arg(&path)
        .arg("ic3")
        .output()
        .expect("ric3 must be on PATH for explicitly enabled solver tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == expected),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn proves_counter_assertion_and_reaches_cover() {
    let model = fixture("counter");
    ric3(
        &model,
        ExportOptions {
            assertion: Some(0),
            ..Default::default()
        },
        "UNSAT",
    );
    ric3(
        &model,
        ExportOptions {
            cover: Some(0),
            ..Default::default()
        },
        "SAT",
    );
}

#[test]
fn proves_fifo_capacity_assertion() {
    let model = fixture("fifo_stage");
    let index = (&model.assertions)
        .into_iter()
        .position(|property| property.name == "fifo_stage_tb::assert[8]")
        .expect("stable flattened property order: three stage assertions and one FIFO assertion, then five top assertions; the last checks capacity matches the FIFO");
    ric3(
        &model,
        ExportOptions {
            assertion: Some(index),
            ..Default::default()
        },
        "UNSAT",
    );
}

#[test]
fn finds_fifo_capacity_assertion_failure() {
    let model = fixture("fifo_stage_fail_assert");
    let index = (&model.assertions).into_iter().position(|property| property.name == "fifo_stage_tb::assert[9]").expect("stable flattened property order: three stage assertions and one FIFO assertion, then six top assertions");
    ric3(
        &model,
        ExportOptions {
            assertion: Some(index),
            ..Default::default()
        },
        "SAT",
    );
}
