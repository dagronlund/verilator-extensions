use std::{fs, path::PathBuf};

use parser_circt::lexer::{
    Lexer,
    error::LexerErrorKind,
    position::LexerPosition,
    symbols::LexerSymbol,
    token::{LexerToken, LexerTokenKind},
};

fn syntax(source: &str) -> Vec<(LexerTokenKind, String)> {
    let mut lexer = Lexer::new(7, source);
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next_syntax().unwrap() {
        tokens.push((token.kind, token.to_string()));
    }
    tokens
}

#[test]
fn custom_and_generic_operations() {
    use LexerTokenKind::*;

    assert_eq!(
        syntax("%r = comb.add %a, %b : i32"),
        vec![
            (ValueIdentifier, "%r".into()),
            (Symbol(LexerSymbol::Equal), "=".into()),
            (Identifier, "comb.add".into()),
            (ValueIdentifier, "%a".into()),
            (Symbol(LexerSymbol::Comma), ",".into()),
            (ValueIdentifier, "%b".into()),
            (Symbol(LexerSymbol::Colon), ":".into()),
            (Identifier, "i32".into()),
        ]
    );
    assert_eq!(
        syntax(r#"%r = "comb.add"(%a, %b) : (i32, i32) -> i32"#),
        vec![
            (ValueIdentifier, "%r".into()),
            (Symbol(LexerSymbol::Equal), "=".into()),
            (String, "\"comb.add\"".into()),
            (Symbol(LexerSymbol::OpenParen), "(".into()),
            (ValueIdentifier, "%a".into()),
            (Symbol(LexerSymbol::Comma), ",".into()),
            (ValueIdentifier, "%b".into()),
            (Symbol(LexerSymbol::CloseParen), ")".into()),
            (Symbol(LexerSymbol::Colon), ":".into()),
            (Symbol(LexerSymbol::OpenParen), "(".into()),
            (Identifier, "i32".into()),
            (Symbol(LexerSymbol::Comma), ",".into()),
            (Identifier, "i32".into()),
            (Symbol(LexerSymbol::CloseParen), ")".into()),
            (Symbol(LexerSymbol::Arrow), "->".into()),
            (Identifier, "i32".into()),
        ]
    );
}

#[test]
fn identifiers_and_contextual_keywords() {
    use LexerTokenKind::*;

    assert_eq!(
        syntax(
            r#"%0#1 %name-with.$_9 ^bb0 !hw.array #loc2 @outer::@"inner-name" module attributes loc i32 si8 ui16"#
        ),
        vec![
            (ValueIdentifier, "%0".into()),
            (AttributeIdentifier, "#1".into()),
            (ValueIdentifier, "%name-with.$_9".into()),
            (BlockIdentifier, "^bb0".into()),
            (TypeIdentifier, "!hw.array".into()),
            (AttributeIdentifier, "#loc2".into()),
            (SymbolIdentifier, "@outer".into()),
            (Symbol(LexerSymbol::Colon), ":".into()),
            (Symbol(LexerSymbol::Colon), ":".into()),
            (SymbolIdentifier, "@\"inner-name\"".into()),
            (Identifier, "module".into()),
            (Identifier, "attributes".into()),
            (Identifier, "loc".into()),
            (Identifier, "i32".into()),
            (Identifier, "si8".into()),
            (Identifier, "ui16".into()),
        ]
    );
    assert_eq!(
        syntax("%12abc foo-bar"),
        vec![
            (ValueIdentifier, "%12".into()),
            (Identifier, "abc".into()),
            (Identifier, "foo".into()),
            (Symbol(LexerSymbol::Minus), "-".into()),
            (Identifier, "bar".into()),
        ]
    );
}

#[test]
fn numbers_preserve_spelling_and_have_separate_signs() {
    use LexerTokenKind::*;

    assert_eq!(
        syntax("-42 +1.25e-3 0x7FF0000000000000 1. 2.E+4 0xi32"),
        vec![
            (Symbol(LexerSymbol::Minus), "-".into()),
            (Integer, "42".into()),
            (Symbol(LexerSymbol::Plus), "+".into()),
            (Float, "1.25e-3".into()),
            (Integer, "0x7FF0000000000000".into()),
            (Float, "1.".into()),
            (Float, "2.E+4".into()),
            (Integer, "0".into()),
            (Identifier, "xi32".into()),
        ]
    );
    let huge = "9".repeat(1000);
    assert_eq!(syntax(&huge), vec![(Integer, huge)]);
}

#[test]
fn punctuation_and_resource_metadata() {
    use LexerTokenKind::*;

    let tokens = syntax("-> ... : , = < > ( ) { } [ ] + - * / ? | {-# #-}");
    assert_eq!(
        tokens.into_iter().map(|(kind, _)| kind).collect::<Vec<_>>(),
        vec![
            Symbol(LexerSymbol::Arrow),
            Symbol(LexerSymbol::Ellipsis),
            Symbol(LexerSymbol::Colon),
            Symbol(LexerSymbol::Comma),
            Symbol(LexerSymbol::Equal),
            Symbol(LexerSymbol::Less),
            Symbol(LexerSymbol::Greater),
            Symbol(LexerSymbol::OpenParen),
            Symbol(LexerSymbol::CloseParen),
            Symbol(LexerSymbol::OpenBrace),
            Symbol(LexerSymbol::CloseBrace),
            Symbol(LexerSymbol::OpenSquare),
            Symbol(LexerSymbol::CloseSquare),
            Symbol(LexerSymbol::Plus),
            Symbol(LexerSymbol::Minus),
            Symbol(LexerSymbol::Star),
            Symbol(LexerSymbol::Slash),
            Symbol(LexerSymbol::Question),
            Symbol(LexerSymbol::VerticalBar),
            Symbol(LexerSymbol::FileMetadataBegin),
            Symbol(LexerSymbol::FileMetadataEnd),
        ]
    );
    // Nested types need two closing angle tokens, not a shift operator.
    assert_eq!(
        syntax(">>"),
        vec![
            (Symbol(LexerSymbol::Greater), ">".into()),
            (Symbol(LexerSymbol::Greater), ">".into()),
        ]
    );
}

#[test]
fn strings_and_quoted_symbols_keep_mlir_escapes() {
    for source in [
        r#""""#,
        r#""hello\n\t\"\\\00\Af""#,
        r#"@"std_pkg::std_is_reset_active""#,
        r#""Unicode λ // is text""#,
        "\"embedded\0nul\"",
    ] {
        let tokens = syntax(source);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].1, source);
    }
}

#[test]
fn positions_and_lossless_trivia() {
    let source = "// λ\r\n\t%a = \"β\"\rfoo\n// eof";
    let tokens = Lexer::new(7, source)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        tokens
            .into_iter()
            .map(|t| t.to_string())
            .collect::<String>(),
        source
    );

    let mut lexer = Lexer::new(7, source);
    let value = lexer.next_syntax().unwrap().unwrap();
    assert_eq!(
        value.position,
        LexerPosition {
            file_id: 7,
            index: 8,
            line: 2,
            column: 2,
            length: 2
        }
    );
    lexer.next_syntax().unwrap().unwrap(); // =
    let string = lexer.next_syntax().unwrap().unwrap();
    assert_eq!(string.position.column, 7);
    assert_eq!(string.position.length, 4);
    let foo = lexer.next_syntax().unwrap().unwrap();
    assert_eq!(foo.position.line, 3);
    assert_eq!(foo.position.column, 1);
    assert!(lexer.next_syntax().unwrap().is_none());
}

#[test]
fn errors_are_positioned_and_stop_iteration() {
    for invalid in [
        "`",
        "é",
        "%",
        "!",
        "^",
        "#",
        "@0",
        ".",
        "..",
        "\"unfinished",
        "@\"unfinished",
        "\"line\nbreak\"",
        "\"line\x0bbreak\"",
        "\"line\x0cbreak\"",
        r#""\q""#,
        r#""\x41""#,
        r#""\A""#,
        "\"trailing\\",
    ] {
        let source = format!("// comment\n  {invalid}");
        let mut lexer = Lexer::new(9, &source);
        let error = lexer.next_syntax().unwrap_err();
        assert_eq!(
            error.kind,
            LexerErrorKind::UnexpectedCharacter,
            "{invalid:?}"
        );
        assert_eq!(error.position.file_id, 9);
        assert_eq!(error.position.line, 2);
        assert_eq!(error.position.column, 3);
        assert!(error.position.length > 0);
        assert!(source.get(error.position.range()).is_some());
        assert!(lexer.next().is_none());
        assert!(lexer.next_syntax().unwrap().is_none());
    }
}

#[test]
fn empty_input_and_repeated_eof() {
    let mut lexer = Lexer::new(0, "");
    assert!(lexer.next().is_none());
    assert!(lexer.next().is_none());
    assert!(lexer.next_syntax().unwrap().is_none());
}

#[test]
fn tokens_outlive_the_source() {
    let token: LexerToken = {
        let source = String::from("comb.add");
        Lexer::new(0, &source).next().unwrap().unwrap()
    };
    assert_eq!(token.to_string(), "comb.add");
}

/// Generate the MLIR fixtures with `./test-circt.sh`, then run the lexer test:
/// `cargo test -p parser-circt --test lexer lexer_round_trips_every_mlir_fixture -- --nocapture`
#[test]
fn lexer_round_trips_every_mlir_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests");
    let mut directories = vec![root.clone()];
    let mut paths = Vec::new();
    // Include hidden and gitignored build directories; do not follow directory
    // symlinks, which may escape the corpus or introduce cycles.
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                directories.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "mlir")
                && path.is_file()
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no .mlir fixtures found under {}; run ./test-circt.sh to generate them",
        root.display()
    );
    let mut total_tokens = 0;
    let mut total_bytes = 0;
    for (file_id, path) in (&paths).into_iter().enumerate() {
        let source =
            fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut reconstructed = Vec::with_capacity(source.len());
        let mut syntax_tokens = 0;
        for token in Lexer::new(file_id, &source) {
            let token = token.unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(token.position.index, reconstructed.len());
            assert_eq!(token.position.file_id, file_id);
            assert_eq!(token.position.length, token.text.len());
            assert!(!token.text.is_empty());
            reconstructed.extend_from_slice(&token.text);
            if !token.kind.is_whitespace() {
                syntax_tokens += 1;
            }
            total_tokens += 1;
        }
        assert!(
            reconstructed == source.as_bytes(),
            "{}: round trip mismatch",
            path.display()
        );
        let mut lexer = Lexer::new(file_id, &source);
        let mut actual_syntax_tokens = 0;
        while lexer.next_syntax().unwrap().is_some() {
            actual_syntax_tokens += 1;
        }
        assert_eq!(actual_syntax_tokens, syntax_tokens, "{}", path.display());
        total_bytes += source.len();
        println!("{}: {} bytes", path.display(), source.len());
    }
    println!(
        "Lexed {} MLIR files: {total_bytes} bytes, {total_tokens} tokens; all round trips exact",
        paths.len()
    );
}
