use std::{fs, path::PathBuf};

use parser_circt::lexer::Lexer;

/// Generate the MLIR fixtures with `./test-circt.sh`, then run the lexer test:
/// `cargo test -p parser-circt --test lexer_fixtures -- --nocapture`
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
