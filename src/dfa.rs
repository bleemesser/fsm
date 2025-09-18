use std::collections::BTreeSet;

use anyhow::Result;
use bimap::BiMap;

use crate::parser;

#[derive(Debug, Clone)]
pub struct StateInfo {
    pub label: Option<String>,
    pub accept: bool,
}

#[derive(Debug)]
pub struct DFA {
    pub name: String,
    pub description: Option<String>,

    pub alphabet: BiMap<char, usize>,     // char <-> alphabet index
    pub state_keys: BiMap<String, usize>, // state key <-> state index

    pub start_state_idx: usize,
    pub accept_state_indices: BTreeSet<usize>,

    pub transition_table: Vec<Vec<usize>>, // state_idx x alphabet_idx -> state_idx

    pub state_properties: Vec<StateInfo>, // index -> state properties
}

impl DFA {
    /// Parses a DFA from a YAML string specification.
    pub fn from_yaml(yaml_content: &str) -> Result<DFA> {
        parser::from_yaml(yaml_content)
    }

    /// Runs the DFA on the given input string and returns true if accepted, false otherwise.
    pub fn run(&self, input: &str) -> bool {
        let mut current_state = self.start_state_idx;

        for c in input.chars() {
            if let Some(&alphabet_idx) = self.alphabet.get_by_left(&c) {
                current_state = self.transition_table[current_state][alphabet_idx];
            } else {
                return false;
            }
        }

        self.accept_state_indices.contains(&current_state)
    }

    /// Prints a human-readable representation of the DFA's transition table.
    pub fn print_transition_table(&self) {
        println!("DFA: {}", self.name);

        let alphabet_size = self.alphabet.len();
        let mut alphabet_header: Vec<char> = vec![' '; alphabet_size];
        for (c, &idx) in self.alphabet.iter() {
            if c == &' ' {
                alphabet_header[idx] = '␣'; // Use a special symbol for space
            } else

            if idx < alphabet_header.len() {
                alphabet_header[idx] = *c;
            }
        }

        const PREFIX_WIDTH: usize = 4; // "--> " or "    "
        const STATE_COL_WIDTH: usize = 10; // 8 chars for key + 1 for '*' + 1 space
        const CELL_WIDTH: usize = 9; // 8 chars for key + 1 space

        print!("{:<PREFIX_WIDTH$}", ""); // padding for the prefix column
        print!("{:<STATE_COL_WIDTH$}", "STATE");
        for c in &alphabet_header {
            print!("{:<CELL_WIDTH$}", c);
        }
        println!();

        // Print Rows
        for (src_idx, row) in self.transition_table.iter().enumerate() {
            let prefix = if src_idx == self.start_state_idx {
                "--> "
            } else {
                "    "
            };
            print!("{:<PREFIX_WIDTH$}", prefix);

            let state_key = self
                .state_keys
                .get_by_right(&src_idx)
                .map_or("ERR", |s| s.as_str());

            // truncate state key to 8 characters
            let trunc_key = if state_key.len() > 8 {
                &state_key[..8]
            } else {
                state_key
            };

            let state_display = format!(
                "{}{}",
                trunc_key,
                if self.accept_state_indices.contains(&src_idx) {
                    "*"
                } else {
                    ""
                }
            );

            print!("{:<STATE_COL_WIDTH$}", state_display);

            for &dest_idx in row {
                let dest_key = self
                    .state_keys
                    .get_by_right(&dest_idx)
                    .map_or("ERR", |s| s.as_str());
                // truncate dest key to 8 characters
                let trunc_dest_key = if dest_key.len() > 8 {
                    &dest_key[..8]
                } else {
                    dest_key
                };

                print!("{:<CELL_WIDTH$}", trunc_dest_key);
            }
            println!();
        }
    }
}
