use anyhow::Result;
use bimap::BiMap;

use crate::parser::{self, Fsm};

#[derive(Debug, Clone)]
pub struct StateInfo {
    pub label: Option<String>,
    pub accept: bool,
}

#[derive(Debug)]
pub struct Dfa {
    pub name: String,
    pub description: Option<String>,

    pub alphabet: BiMap<char, usize>,     // char <-> alphabet index
    pub state_keys: BiMap<String, usize>, // state key <-> state index

    pub start_state_idx: usize,
    // [state1_is_accept, state2_is_accept, ...]
    pub accept_states: Vec<bool>,

    // (state_idx * alphabet_len) + alphabet_idx -> next_state_idx
    pub transition_table: Vec<usize>,

    pub state_properties: Vec<StateInfo>, // index -> state properties
}

impl Dfa {
    /// Parses a DFA from a YAML string specification.
    pub fn from_yaml(yaml_content: &str) -> Result<Fsm> {
        parser::from_yaml(yaml_content)
    }

    /// Runs the DFA on the given input string and returns true if accepted, false otherwise.
    pub fn run<I>(&self, input: I) -> bool
    where
        I: IntoIterator<Item = char>,
    {
        let mut current_state = self.start_state_idx;

        let alphabet_size = self.alphabet.len();

        let mut prev_char: char;
        let mut prev_index: usize;
        let mut iter = input.into_iter();

        // handle the first character separately to avoid using Option in the loop
        if let Some(c) = iter.next() {
            if let Some(&idx) = self.alphabet.get_by_left(&c) {
                prev_char = c;
                prev_index = idx;
                current_state = self.transition_table[(current_state * alphabet_size) + idx];
            } else {
                return false;
            }
        } else {
            return self.accept_states[current_state];
        }

        // handle remaining characters
        for c in iter {
            let alphabet_idx = if c == prev_char {
                prev_index
            } else {
                if let Some(&idx) = self.alphabet.get_by_left(&c) {
                    prev_char = c;
                    prev_index = idx;
                    idx
                } else {
                    return false;
                }
            };

            current_state = self.transition_table[(current_state * alphabet_size) + alphabet_idx];
        }

        self.accept_states[current_state]
    }

    /// Prints a human-readable representation of the DFA's transition table.
    pub fn print_transition_table(&self) {
        println!("DFA: {}", self.name);

        let alphabet_size = self.alphabet.len();
        let mut alphabet_header: Vec<char> = vec![' '; alphabet_size];
        for (c, &idx) in self.alphabet.iter() {
            if c == &' ' {
                alphabet_header[idx] = '␣'; // Use a special symbol for space
            } else if idx < alphabet_header.len() {
                alphabet_header[idx] = *c;
            }
        }
        const CHARS_FOR_KEY: usize = 18;
        const PREFIX_WIDTH: usize = 4; // "--> " or "    "
        const STATE_COL_WIDTH: usize = CHARS_FOR_KEY + 2; // chars for key + 1 for '*' + 1 space
        const CELL_WIDTH: usize = CHARS_FOR_KEY + 1; // chars for key + 1 space

        print!("{:<PREFIX_WIDTH$}", ""); // padding for the prefix column
        print!("{:<STATE_COL_WIDTH$}", "STATE");
        for c in &alphabet_header {
            print!("{:<CELL_WIDTH$}", c);
        }
        println!();

        // Print Rows
        for src_idx in 0..self.state_keys.len() {
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

            // truncate state key
            let trunc_key = if state_key.len() > CHARS_FOR_KEY {
                &state_key[..CHARS_FOR_KEY]
            } else {
                state_key
            };

            let state_display = format!(
                "{}{}",
                trunc_key,
                if self.accept_states[src_idx] { "*" } else { "" }
            );

            print!("{:<STATE_COL_WIDTH$}", state_display);

            for alpha_idx in 0..alphabet_size {
                let dest_idx = self.transition_table[(src_idx * alphabet_size) + alpha_idx];

                let dest_key = self
                    .state_keys
                    .get_by_right(&dest_idx)
                    .map_or("ERR", |s| s.as_str());
                // truncate dest key
                let trunc_dest_key = if dest_key.len() > CHARS_FOR_KEY {
                    &dest_key[..CHARS_FOR_KEY]
                } else {
                    dest_key
                };

                print!("{:<CELL_WIDTH$}", trunc_dest_key);
            }
            println!();
        }
    }
}
