use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
};

use num_bigint::BigUint;

use crate::{
    ast::{
        DataTypeKind, Design, Direction, Edge, PropertyKind, VariableId, VariableKind,
        collect::CollectAccesses, range::Range,
    },
    document::AstDocument,
};

#[test]
fn parses_and_visits_recursive_ast_nodes() {
    let input = r#"
    {
      "type": "NETLIST",
      "name": "$root",
      "modulesp": [
        {
          "type": "MODULE",
          "name": "counter",
          "stmtsp": [
            {"type": "VAR", "name": "count", "width": 4}
          ]
        }
      ],
      "topScopep": "(A)"
    }
    "#;

    let document = AstDocument::from_reader(input.as_bytes()).unwrap();

    assert_eq!(document.root.node_type, "NETLIST");
    assert_eq!(document.root.name.as_deref(), Some("$root"));
    assert_eq!(document.node_count(), 3);
    assert_eq!(document.node_type_counts().get("MODULE"), Some(&1));
    assert_eq!(document.node_type_counts().get("VAR"), Some(&1));
}

#[test]
fn requires_an_ast_node_at_the_root() {
    let error = AstDocument::from_reader(r#"{"name":"missing type"}"#.as_bytes()).unwrap_err();
    assert!(error.to_string().contains("type"));
}

#[test]
fn ast_ranges_preserve_index_order_without_allocating() {
    let descending = Range { left: 7, right: 0 };
    let ascending = Range { left: 0, right: 7 };

    assert_eq!(descending.swap(), ascending);
    assert_eq!(ascending.swap(), descending);
    assert_eq!(
        descending.into_iter().collect::<Vec<_>>(),
        (0..=7).rev().collect::<Vec<_>>()
    );
}

#[test]
fn verilator_wire_dtypes_and_variables_are_supported() {
    let input = r#"
    {
      "type":"NETLIST",
      "nodesp":[
        {"type":"BASICDTYPE","addr":"(D1)","name":"wire","keyword":"wire"},
        {"type":"BASICDTYPE","addr":"(D8)","name":"wire","keyword":"wire","range":"7:0"},
        {"type":"VAR","addr":"(C)","name":"clk","origName":"clk","dtypep":"(D1)","direction":"INPUT","varType":"PORT"},
        {"type":"VAR","addr":"(Q)","name":"state","origName":"state","dtypep":"(D8)","varType":"WIRE"},
        {"type":"SENTREE","addr":"(S)","sensesp":[
          {"type":"SENITEM","edgeType":"POS","sensp":{"type":"VARREF","varp":"(C)","dtypep":"(D1)","access":"RD"}}
        ]},
        {"type":"SCOPE","addr":"(P)","name":"TOP","blocksp":[
          {"type":"ACTIVE","name":"sequent","sentreep":"(S)","stmtsp":[
            {"type":"ASSIGN","lhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D8)","access":"WR"},"rhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D8)","access":"RD"}}
          ]}
        ]}
      ]
    }
    "#;
    let document = AstDocument::from_reader(input.as_bytes()).unwrap();
    let design = Design::try_from(&document).unwrap();

    let clock_dtype = design.data_type(design.variables[0].dtype);
    assert_eq!(clock_dtype.width, 1);
    assert_eq!(clock_dtype.indices, Range { left: 0, right: 0 });
    assert_eq!(design.data_type_rank(design.variables[0].dtype), 0);
    assert_eq!(design.variable_rank(VariableId(0)), 0);
    assert!(design.variables[0].top_level);
    let state_dtype = design.data_type(design.variables[1].dtype);
    assert_eq!(state_dtype.width, 8);
    assert_eq!(state_dtype.indices, Range { left: 0, right: 7 });
    assert_eq!(design.data_type_rank(design.variables[1].dtype), 1);
    assert_eq!(design.variable_rank(VariableId(1)), 1);
    assert_eq!(design.variables[1].kind, VariableKind::Wire);
}

#[test]
fn rejects_unsupported_statements() {
    let input = r#"
    {
      "type":"NETLIST",
      "nodesp":[
        {"type":"BASICDTYPE","addr":"(D)"},
        {"type":"VAR","addr":"(C)","name":"clk","origName":"clk","dtypep":"(D)","direction":"INPUT","varType":"PORT"},
        {"type":"SENTREE","addr":"(S)","sensesp":[
          {"type":"SENITEM","edgeType":"POS","sensp":{"type":"VARREF","varp":"(C)","dtypep":"(D)","access":"RD"}}
        ]},
        {"type":"SCOPE","addr":"(P)","name":"TOP","blocksp":[
          {"type":"ACTIVE","name":"sequent","sentreep":"(S)","stmtsp":[
            {"type":"UNSUPPORTED","loc":"test.sv,1:1,1:2"}
          ]}
        ]}
      ]
    }
    "#;
    let document = AstDocument::from_reader(input.as_bytes()).unwrap();
    let error = Design::try_from(&document).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unsupported procedural statement")
    );
    assert!(error.to_string().contains("test.sv"));
}

#[test]
fn typed_ast_is_owned_and_resolves_counter_metadata() {
    let design = {
        let document = AstDocument::from_path(build_fixture("counter")).unwrap();
        Design::try_from(&document).unwrap()
    };

    assert_eq!(design.sensitivity_domains.len(), 1);
    assert_eq!(design.sensitivity_domains[0].domain.name, "clk");
    assert_eq!(design.sensitivity_domains[0].domain.edge, Edge::Positive);
    assert_eq!(
        design
            .variable(design.sensitivity_domains[0].variable)
            .display_name(),
        "clk"
    );
    assert_eq!(design.initial.len(), 1);
    assert_eq!(design.initial[0].source.node_type, "INITIALSTATIC");
    assert!(design.combinational.is_empty());
    assert!(!design.sequential.is_empty());
    assert!(!design.data_types.is_empty());

    let reset = (&design.variables)
        .into_iter()
        .find(|variable| variable.display_name() == "reset_n")
        .unwrap();
    assert_eq!(reset.direction, Direction::Input);
    assert!(reset.top_level);
    assert!(reset.source.address.is_some());

    let assertion = (&design.variables)
        .into_iter()
        .filter_map(|variable| variable.property.as_ref())
        .find(|property| property.kind == PropertyKind::Assertion)
        .unwrap();
    assert_eq!(assertion.kind, PropertyKind::Assertion);
    assert_eq!(assertion.name, "assert_holds_when_disabled");
    assert!(
        (&design.variables)
            .into_iter()
            .any(|variable| variable.sampled_value.is_some())
    );
}

#[test]
fn preserved_sva_wires_resolve_all_property_kinds() {
    let document = AstDocument::from_path(build_fixture("counter")).unwrap();
    let design = Design::try_from(&document).unwrap();
    let properties: Vec<_> = (&design.variables)
        .into_iter()
        .filter_map(|variable| variable.property.as_ref())
        .collect();

    assert_eq!(properties.len(), 3);
    for (kind, name) in [
        (PropertyKind::Assertion, "assert_holds_when_disabled"),
        (PropertyKind::Assumption, "assume_no_overflow"),
        (PropertyKind::Cover, "cover_saturation"),
    ] {
        let matching: Vec<_> = (&properties)
            .into_iter()
            .filter(|property| property.kind == kind)
            .map(|property| property.name.as_str())
            .collect();
        assert_eq!(matching, vec![name]);
    }
}

#[test]
fn static_initializers_precede_initial_blocks_with_omitted_or_empty_names() {
    let input = r#"
    {"type":"NETLIST","nodesp":[
      {"type":"BASICDTYPE","addr":"(D)"},
      {"type":"VAR","addr":"(C)","name":"clk","dtypep":"(D)","direction":"INPUT","varType":"PORT"},
      {"type":"VAR","addr":"(Q)","name":"state","dtypep":"(D)","varType":"VAR"},
      {"type":"SENTREE","addr":"(S)","sensesp":[
        {"type":"SENITEM","edgeType":"POS","sensp":{"type":"VARREF","varp":"(C)","dtypep":"(D)","access":"RD"}}
      ]},
      {"type":"SCOPE","addr":"(P)","name":"TOP","blocksp":[
        {"type":"ACTIVE","stmtsp":[{"type":"INITIAL","stmtsp":[
          {"type":"ASSIGN","lhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D)","access":"WR"},"rhsp":{"type":"CONST","name":"1'h1","dtypep":"(D)"}}
        ]}]},
        {"type":"ACTIVE","name":"","stmtsp":[{"type":"INITIALSTATIC","stmtsp":[
          {"type":"ASSIGN","lhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D)","access":"WR"},"rhsp":{"type":"CONST","name":"1'h0","dtypep":"(D)"}}
        ]}]},
        {"type":"ACTIVE","name":"sequent","sentreep":"(S)","stmtsp":[
          {"type":"ASSIGN","lhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D)","access":"WR"},"rhsp":{"type":"VARREF","varp":"(Q)","dtypep":"(D)","access":"RD"}}
        ]}
      ]}
    ]}
    "#;
    let document = AstDocument::from_reader(input.as_bytes()).unwrap();
    let design = Design::try_from(&document).unwrap();
    assert_eq!(design.initial.len(), 2);
    assert_eq!(design.initial[0].source.node_type, "INITIALSTATIC");
    assert_eq!(design.initial[1].source.node_type, "INITIAL");
    assert!(design.combinational.is_empty());
}

#[test]
fn typed_ast_round_trips_through_ron() {
    let document = AstDocument::from_path(build_fixture("counter")).unwrap();
    let design = Design::try_from(&document).unwrap();

    let serialized =
        ron::ser::to_string_pretty(&design, ron::ser::PrettyConfig::default()).unwrap();
    let deserialized: Design = ron::from_str(&serialized).unwrap();

    assert_eq!(deserialized, design);
    assert!(serialized.contains("shadow_registers"));
}

#[test]
fn fixed_width_dtype_fixture_preserves_structural_metadata() {
    let document = AstDocument::from_path(build_fixture("data_types")).unwrap();
    let union_node = document
        .nodes()
        .into_iter()
        .find(|node| node.node_type == "UNIONDTYPE")
        .unwrap();
    assert_eq!(
        union_node.children("membersp").len(),
        2,
        "membersp: {:?}",
        union_node.field("membersp")
    );
    let design = Design::try_from(&document).unwrap();

    for (name, width, signed) in [
        ("byte", 8, true),
        ("shortint", 16, true),
        ("int", 32, true),
        ("integer", 32, true),
        ("longint", 64, true),
        ("time", 64, false),
    ] {
        let dtype = (&design.data_types)
            .into_iter()
            .find(|dtype| dtype.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {name} dtype"));
        assert_eq!(dtype.width, width, "{name} width");
        assert_eq!(dtype.signed, signed, "{name} signedness");
        let DataTypeKind::Basic { packed } = dtype.kind else {
            panic!("expected {name} to resolve to BASICDTYPE");
        };
        assert_eq!(
            packed,
            Range {
                left: isize::try_from(width - 1).unwrap(),
                right: 0,
            }
        );
    }
    for name in ["bit", "logic"] {
        assert!((&design.data_types).into_iter().any(|dtype| {
            dtype.name.as_deref() == Some(name)
                && dtype.width == 1
                && match dtype.kind {
                    DataTypeKind::Basic { .. } => true,
                    _ => false,
                }
        }));
    }

    for name in ["signed_byte_t", "alias_t", "VALUE_T"] {
        let dtype = (&design.data_types)
            .into_iter()
            .find(|dtype| dtype.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("missing {name} alias dtype"));
        assert_eq!(dtype.width, 8);
        let DataTypeKind::Alias { target } = dtype.kind else {
            panic!("expected {name} to preserve alias metadata");
        };
        assert_eq!(design.data_type(target).width, 8);
    }

    let record = (&design.data_types)
        .into_iter()
        .find(|dtype| {
            dtype
                .name
                .as_deref()
                .is_some_and(|name| name.contains("record_t__struct"))
        })
        .unwrap();
    let DataTypeKind::PackedStruct { members } = &record.kind else {
        panic!("expected packed struct metadata");
    };
    assert_eq!(record.width, 16);
    assert_eq!(members[0].name, "payload");
    assert_eq!(members[0].lsb, 8);
    assert_eq!(members[1].name, "state");
    assert_eq!(members[1].lsb, 6);
    assert_eq!(members[2].name, "flags");
    assert_eq!(members[2].lsb, 0);
    for member in members {
        let DataTypeKind::Alias { target } = design.data_type(member.dtype).kind else {
            panic!("expected standalone MEMBERDTYPE alias metadata");
        };
        assert_eq!(design.data_type(target).width, member.width);
    }

    assert!(
        (&design.data_types)
            .into_iter()
            .any(|dtype| match &dtype.kind {
                DataTypeKind::Enum { variants, .. } => {
                    variants
                        .into_iter()
                        .map(|variant| variant.value.value.clone())
                        .collect::<Vec<_>>()
                        == [0u8, 1, 2].map(BigUint::from)
                }
                _ => false,
            })
    );
    assert!(
        (&design.data_types)
            .into_iter()
            .any(|dtype| match dtype.kind {
                DataTypeKind::PackedArray {
                    declared: Range { left: 0, right: 1 },
                    ..
                } => true,
                _ => false,
            })
    );
    assert!(
        (&design.data_types)
            .into_iter()
            .any(|dtype| match dtype.kind {
                DataTypeKind::PackedArray {
                    declared: Range { left: 1, right: 0 },
                    ..
                } => true,
                _ => false,
            })
    );
    assert!((&design.data_types).into_iter().any(|dtype| {
        match dtype.kind {
            DataTypeKind::PackedUnion { .. } => dtype.width == 16,
            _ => false,
        }
    }));
    let matrix_dtype = design
        .data_types
        .iter()
        .position(|dtype| dtype.name.as_deref() == Some("matrix_t"))
        .map(crate::ast::DataTypeId)
        .unwrap();
    let matrix = design.data_type(matrix_dtype);
    assert_eq!(matrix.width, 96);
    assert_eq!(matrix.indices, Range { left: 0, right: 95 });
    assert_eq!(
        matrix.unpacked.as_ref().unwrap().indices,
        Range { left: 0, right: 2 }
    );
    assert_eq!(matrix.unpacked.as_ref().unwrap().element_width, 32);
    let DataTypeKind::Alias { target: outer } = matrix.kind else {
        panic!("expected matrix_t alias");
    };
    let DataTypeKind::UnpackedArray {
        element: row_alias,
        declared: outer_range,
    } = design.data_type(outer).kind
    else {
        panic!("expected outer unpacked array");
    };
    assert_eq!(outer_range, Range { left: 0, right: 2 });
    let DataTypeKind::Alias { target: inner } = design.data_type(row_alias).kind else {
        panic!("expected row_t alias");
    };
    let DataTypeKind::UnpackedArray {
        element: nested_record,
        declared: inner_range,
    } = design.data_type(inner).kind
    else {
        panic!("expected inner unpacked array");
    };
    assert_eq!(inner_range, Range { left: 1, right: 0 });
    assert_eq!(design.data_type(nested_record).width, 16);

    assert_eq!(design.data_type_rank(nested_record), 3);
    assert_eq!(design.data_type_rank(row_alias), 4);
    assert_eq!(design.data_type_rank(inner), 4);
    assert_eq!(design.data_type_rank(outer), 5);
    assert_eq!(design.data_type_rank(matrix_dtype), 5);
    let matrix_variable = design
        .variables
        .iter()
        .position(|variable| variable.display_name() == "matrix_value")
        .map(VariableId)
        .unwrap();
    assert_eq!(design.variable_rank(matrix_variable), 5);

    let serialized = ron::to_string(&design).unwrap();
    let deserialized: Design = ron::from_str(&serialized).unwrap();
    assert_eq!(deserialized, design);
}

#[test]
fn typed_ast_preserves_ranges_and_arrays() {
    let signed_document = AstDocument::from_path(build_fixture("signed_operations")).unwrap();
    let signed_design = Design::try_from(&signed_document).unwrap();
    assert!((&signed_design.data_types).into_iter().any(|dtype| {
        match dtype.kind {
            DataTypeKind::Basic { packed } => packed.left != packed.right,
            _ => false,
        }
    }));

    let array_document = AstDocument::from_path(build_fixture("multidim_arrays")).unwrap();
    let array_design = Design::try_from(&array_document).unwrap();
    assert!(
        (&array_design.data_types)
            .into_iter()
            .any(|dtype| dtype.unpacked.is_some())
    );
    assert!((&array_design.data_types).into_iter().any(|dtype| {
        matches!(
            dtype.kind,
            DataTypeKind::Basic {
                packed: Range {
                    left: -1,
                    right: -4,
                },
            }
        )
    }));
}

#[test]
fn combinational_array_assignments_report_block_accesses() {
    let document = AstDocument::from_path(build_fixture("combinational_loops")).unwrap();
    let design = Design::try_from(&document).unwrap();
    assert_eq!(design.combinational.len(), 2);

    let variable_id = |name| {
        design
            .variables
            .iter()
            .position(|variable| variable.display_name() == name)
            .map(VariableId)
            .unwrap()
    };
    let increments = variable_id("counts");
    let total = variable_id("total");

    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    design.combinational[0].collect_accesses(&mut reads, &mut writes);
    assert_eq!(reads, BTreeSet::from([increments]));
    assert_eq!(writes, BTreeSet::from([increments]));

    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    design.combinational[1].collect_accesses(&mut reads, &mut writes);
    assert_eq!(reads, BTreeSet::from([increments]));
    assert_eq!(writes, BTreeSet::from([total]));
}

fn build_fixture(fixture: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join(fixture);
    run_ninja(&directory, fixture, &[]);

    match find_ast(&directory) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("fixture {fixture} is invalid, rebuilding: {error}");
            run_ninja(&directory, fixture, &["-t", "clean"]);
            run_ninja(&directory, fixture, &[]);
            find_ast(&directory).unwrap_or_else(|error| {
                panic!("fixture {fixture} is still invalid after rebuilding: {error}")
            })
        }
    }
}

fn run_ninja(directory: &Path, fixture: &str, arguments: &[&str]) {
    let status = Command::new("ninja")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .status()
        .unwrap_or_else(|error| panic!("failed to run Ninja for fixture {fixture}: {error}"));
    assert!(status.success(), "failed to build fixture {fixture}");
}

fn find_ast(directory: &Path) -> Result<PathBuf, String> {
    let path = directory.join("build/ast.json");
    AstDocument::from_path(&path)
        .map_err(|error| format!("AST {} is invalid: {error}", path.display()))?;
    Ok(path)
}
