use std::collections::BTreeMap;

pub(crate) fn storage_type(width: usize) -> &'static str {
    match width {
        0 | 1 => "bool",
        2..=8 => "u8",
        9..=16 => "u16",
        17..=32 => "u32",
        33..=64 => "u64",
        65..=128 => "u128",
        _ => "num_bigint::BigUint",
    }
}

pub(crate) fn bits_parameters(width: usize) -> String {
    format!("{width}, {}", storage_type(width))
}

pub(crate) fn public_value_type(width: usize) -> String {
    match width {
        0 | 1 => "bool".to_string(),
        2..=8 => "u8".to_string(),
        9..=16 => "u16".to_string(),
        17..=32 => "u32".to_string(),
        33..=64 => "u64".to_string(),
        65..=128 => "u128".to_string(),
        _ => format!("Bits<{}>", bits_parameters(width)),
    }
}

pub(crate) fn public_to_bits(value: &str, width: usize) -> String {
    let width_type = bits_parameters(width);
    if width <= 1 {
        format!("Bits::<{width_type}>::from_bool({value})")
    } else if width <= 8 {
        format!("Bits::<{width_type}>::from_u8({value})")
    } else if width <= 16 {
        format!("Bits::<{width_type}>::from_u16({value})")
    } else if width <= 32 {
        format!("Bits::<{width_type}>::from_u32({value})")
    } else if width <= 64 {
        format!("Bits::<{width_type}>::from_u64({value})")
    } else if width <= 128 {
        format!("Bits::<{width_type}>::from_u128({value})")
    } else {
        format!("{value}.clone()")
    }
}

pub(crate) fn bits_to_public(value: &str, width: usize) -> String {
    if width <= 1 {
        format!("{value}.bit(0)")
    } else if width <= 8 {
        format!("{value}.to_u128() as u8")
    } else if width <= 16 {
        format!("{value}.to_u128() as u16")
    } else if width <= 32 {
        format!("{value}.to_u128() as u32")
    } else if width <= 64 {
        format!("{value}.to_u128() as u64")
    } else if width <= 128 {
        format!("{value}.to_u128()")
    } else {
        format!("{value}.clone()")
    }
}

pub(crate) fn unique_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut used = BTreeMap::new();
    names
        .into_iter()
        .map(|name| unique_name(name, &mut used))
        .collect()
}

pub(crate) fn unique_name(name: String, used: &mut BTreeMap<String, usize>) -> String {
    let count = used.entry(name.clone()).or_default();
    let result = if *count == 0 {
        name
    } else {
        format!("{name}_{}", *count)
    };
    *count += 1;
    result
}

pub(crate) fn snake_identifier(name: &str) -> String {
    let mut result = String::new();
    let mut previous_underscore = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
            previous_underscore = false;
        } else if !previous_underscore && !result.is_empty() {
            result.push('_');
            previous_underscore = true;
        }
    }
    while result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        result.push_str("signal");
    }
    if result.starts_with(|character: char| character.is_ascii_digit()) {
        result.insert_str(0, "signal_");
    }
    if is_rust_keyword(&result) {
        result.push('_');
    }
    result
}

pub(crate) fn type_identifier(name: &str) -> String {
    let snake = snake_identifier(name);
    let mut result = String::new();
    for word in snake.split('_') {
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            result.push(first.to_ascii_uppercase());
            result.extend(characters);
        }
    }
    if result.is_empty() {
        "RtlType".to_string()
    } else {
        result
    }
}

pub(crate) fn screaming_identifier(name: &str) -> String {
    snake_identifier(name).to_ascii_uppercase()
}

pub(crate) fn is_rust_keyword(name: &str) -> bool {
    [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ]
    .contains(&name)
}
