use crate::dfa::Dfa;
use crate::parser::Nfa;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generates a Graphviz DOT file representation of the DFA.
pub fn make_dot(fsm: &Dfa, filename: impl AsRef<Path>) -> Result<()> {
    let mut file = File::create(filename)?;

    writeln!(
        &mut file,
        "digraph \"{}\" {{",
        fsm.name.replace('\"', "\\\"")
    )?;
    writeln!(&mut file, "    rankdir=LR;")?;

    let label = fsm
        .description
        .as_deref()
        .unwrap_or(&fsm.name)
        .replace('\"', "\\\"")
        .replace('\n', "\\n");
    writeln!(&mut file, "    label=\"{}\";", label)?;
    writeln!(&mut file, "    node [shape=circle];")?;

    writeln!(&mut file, "    __start [shape=none, label=\"\"];")?;

    for (idx, props) in fsm.state_properties.iter().enumerate() {
        let state_key = fsm.state_keys.get_by_right(&idx).unwrap_or_else(|| {
            panic!("State index {} not found in state_keys", idx)
        });

        let shape = if fsm.accept_states[idx] {
            "doublecircle"
        } else {
            "circle"
        };

        let label = props
            .label
            .as_deref()
            .unwrap_or(state_key)
            .replace('\"', "\\\"");

        writeln!(
            &mut file,
            "    \"{}\" [label=\"{}\", shape={}];",
            state_key.replace('\"', "\\\""),
            label,
            shape
        )?;
    }

    let start_key = fsm.state_keys.get_by_right(&fsm.start_state_idx).unwrap_or_else(|| {
        panic!(
            "Start state index {} not found in state_keys",
            fsm.start_state_idx
        )
    });
    writeln!(
        &mut file,
        "    __start -> \"{}\";",
        start_key.replace('\"', "\\\"")
    )?;

    let mut transitions: BTreeMap<(usize, usize), BTreeSet<char>> = BTreeMap::new();

    // transition table is now a 1d vec of size (num_states * alphabet_size)
    let alphabet_size = fsm.alphabet.len();
    for (src_idx, row) in fsm.transition_table.chunks(alphabet_size).enumerate() {
        for (alpha_idx, &dest_idx) in row.iter().enumerate() {
            let c = fsm.alphabet.get_by_right(&alpha_idx).unwrap_or_else(|| {
                panic!("Alphabet index {} not found in alphabet", alpha_idx)
            });
            transitions
                .entry((src_idx, dest_idx))
                .or_default()
                .insert(*c);
        }
    }

    for ((src_idx, dest_idx), chars) in transitions {
        let src_key = fsm.state_keys.get_by_right(&src_idx).unwrap_or_else(|| {
            panic!("State index {} not found in state_keys", src_idx)
        });
        let dest_key = fsm.state_keys.get_by_right(&dest_idx).unwrap_or_else(|| {
            panic!("State index {} not found in state_keys", dest_idx)
        });
        let label = format_char_set(&chars);

        writeln!(
            &mut file,
            "    \"{}\" -> \"{}\" [label=\"{}\"];",
            src_key.replace('\"', "\\\""),
            dest_key.replace('\"', "\\\""),
            label.replace('\"', "\\\"")
        )?;
    }

    writeln!(&mut file, "}}")?;
    Ok(())
}

pub fn make_nfa_dot(
    nfa: &Nfa,
    fsm_name: &str,
    description: Option<&str>,
    filename: impl AsRef<Path>,
) -> Result<()> {
    let mut file = File::create(filename)?;

    writeln!(
        &mut file,
        "digraph \"{}\" {{",
        fsm_name.replace('\"', "\\\"")
    )?;
    writeln!(&mut file, "    rankdir=LR;")?;

    let label = description
        .unwrap_or(fsm_name)
        .replace('\"', "\\\"")
        .replace('\n', "\\n");
    writeln!(&mut file, "    label=\"{}\";", label)?;
    writeln!(&mut file, "    node [shape=circle];")?;

    writeln!(&mut file, "    __start [shape=none, label=\"\"];")?;

    for (state_key, &idx) in nfa.nfa_state_keys.iter() {
        let shape = if nfa.nfa_accept_states.contains(&idx) {
            "doublecircle"
        } else {
            "circle"
        };

        writeln!(
            &mut file,
            "    \"{}\" [label=\"{}\", shape={}];",
            state_key.replace('\"', "\\\""),
            state_key.replace('\"', "\\\""),
            shape
        )?;
    }

    let start_key = nfa.nfa_state_keys.get_by_right(&nfa.start_state).unwrap_or_else(|| {
        panic!(
            "Start state index {} not found in nfa_state_keys",
            nfa.start_state
        )
    });
    writeln!(
        &mut file,
        "    __start -> \"{}\";",
        start_key.replace('\"', "\\\"")
    )?;

    // group transitions by (src, dest) to consolidate labels
    let mut transitions_grouped: BTreeMap<(usize, usize), BTreeSet<Option<char>>> = BTreeMap::new();
    for ((src_idx, on_char), dest_states) in &nfa.transitions {
        for &dest_idx in dest_states {
            transitions_grouped
                .entry((*src_idx, dest_idx))
                .or_default()
                .insert(*on_char);
        }
    }

    for ((src_idx, dest_idx), chars) in transitions_grouped {
        let src_key = nfa.nfa_state_keys.get_by_right(&src_idx).unwrap_or_else(|| {
            panic!("State index {} not found in nfa_state_keys", src_idx)
        });
        let dest_key = nfa.nfa_state_keys.get_by_right(&dest_idx).unwrap_or_else(|| {
            panic!("State index {} not found in nfa_state_keys", dest_idx)
        });
        let label_chars: BTreeSet<char> = chars.iter().filter_map(|&c| c).collect();
        let mut label_parts = Vec::new();
        if !label_chars.is_empty() {
            label_parts.push(format_char_set(&label_chars));
        }
        if chars.contains(&None) {
            label_parts.push("ε".to_string());
        }

        let label = label_parts.join(", ");

        writeln!(
            &mut file,
            "    \"{}\" -> \"{}\" [label=\"{}\"];",
            src_key.replace('\"', "\\\""),
            dest_key.replace('\"', "\\\""),
            label.replace('\"', "\\\"")
        )?;
    }

    writeln!(&mut file, "}}")?;
    Ok(())
}

/// Formats a set of characters into a compact, readable string (e.g., "a-c, z, 0-9").
fn format_char_set(chars: &BTreeSet<char>) -> String {
    if chars.is_empty() {
        return " ".to_string();
    }

    let mut parts = Vec::new();
    let mut iter = chars.iter().peekable();

    while let Some(&start) = iter.next() {
        let mut end = start;

        while let Some(&&next) = iter.peek() {
            if (next as u32) == (end as u32) + 1 {
                end = next;
                iter.next();
            } else {
                break;
            }
        }

        if start == end {
            parts.push(format_char(start));
        } else if (end as u32) == (start as u32) + 1 {
            parts.push(format_char(start));
            parts.push(format_char(end));
        } else {
            parts.push(format!("{}-{}", format_char(start), format_char(end)));
        }
    }

    parts.join(", ")
}

/// Formats a single character for display, escaping special DOT characters.
fn format_char(c: char) -> String {
    match c {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        ' ' => "␣".to_string(),
        ',' => "[comma]".to_string(),
        '-' => "[dash]".to_string(),
        _ => c.to_string(),
    }
}
