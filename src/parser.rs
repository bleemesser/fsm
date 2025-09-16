use crate::dfa::{DFA, StateInfo};
use anyhow::Result;
use bimap::BiMap;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize, Debug)]
struct YamlDFA {
    name: String,
    description: Option<String>,
    states: BTreeMap<String, YamlStateProps>,
    alphabet: Vec<YamlSymbolSpecifier>,
    start_state: String,
    transitions: BTreeMap<String, Vec<YamlTransitionMapping>>,
}

#[derive(Deserialize, Debug, Clone, Default)]
struct YamlStateProps {
    #[serde(default)]
    accept: bool,
    label: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
enum YamlSymbolSpecifier {
    Map(YamlRangeMap),
    Literal(String),
}

impl YamlSymbolSpecifier {
    fn to_char_set(&self) -> Result<BTreeSet<char>> {
        let mut char_set = BTreeSet::new();
        match self {
            YamlSymbolSpecifier::Literal(s) => {
                for c in s.chars() {
                    char_set.insert(c);
                }
            }
            YamlSymbolSpecifier::Map(range_map) => {
                if let Some(crange) = &range_map.crange {
                    let parts: Vec<&str> = crange.split("..").collect();
                    if parts.len() != 2 {
                        return Err(anyhow::anyhow!("Invalid character range: {}", crange));
                    }
                    let start = parts[0]
                        .chars()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("Empty start in range: {}", crange))?;
                    let end = parts[1]
                        .chars()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("Empty end in range: {}", crange))?;
                    if start > end {
                        return Err(anyhow::anyhow!(
                            "Start character greater than end in range: {}",
                            crange
                        ));
                    }
                    for c in (start as u8)..=(end as u8) {
                        char_set.insert(c as char);
                    }
                }
                if let Some(nrange) = &range_map.nrange {
                    let parts: Vec<&str> = nrange.split("..").collect();
                    if parts.len() != 2 {
                        return Err(anyhow::anyhow!("Invalid numeric range: {}", nrange));
                    }
                    let start: u8 = parts[0].parse().map_err(|_| {
                        anyhow::anyhow!("Invalid start number in range: {}", nrange)
                    })?;
                    let end: u8 = parts[1]
                        .parse()
                        .map_err(|_| anyhow::anyhow!("Invalid end number in range: {}", nrange))?;
                    if start > end {
                        return Err(anyhow::anyhow!(
                            "Start number greater than end in range: {}",
                            nrange
                        ));
                    }
                    if start > 9 || end > 9 {
                        return Err(anyhow::anyhow!(
                            "Numeric range must be between 0 and 9: {}",
                            nrange
                        ));
                    }
                    for n in start..=end {
                        char_set.insert((b'0' + n) as char);
                    }
                }
            }
        }
        Ok(char_set)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct YamlRangeMap {
    #[serde(default)]
    crange: Option<String>,
    #[serde(default)]
    nrange: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
enum YamlKeyword {
    #[serde(rename = "alphabet")]
    Alphabet,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum YamlTransitionOn {
    Except { except: YamlExceptValue },
    Keyword(YamlKeyword),
    Single(YamlSymbolSpecifier),
    Multiple(Vec<YamlSymbolSpecifier>),
}

impl YamlTransitionOn {
    fn to_char_set(&self, full_alphabet: &BTreeSet<char>) -> Result<BTreeSet<char>> {
        match self {
            YamlTransitionOn::Single(spec) => spec.to_char_set(),
            YamlTransitionOn::Multiple(specs) => {
                let mut char_set = BTreeSet::new();
                for spec in specs {
                    char_set.extend(spec.to_char_set()?);
                }
                Ok(char_set)
            }
            YamlTransitionOn::Keyword(kw) => match kw {
                YamlKeyword::Alphabet => Ok(full_alphabet.clone()),
            },
            YamlTransitionOn::Except { except } => {
                let except_chars = match except {
                    YamlExceptValue::Single(spec) => spec.to_char_set()?,
                    YamlExceptValue::Multiple(specs) => {
                        let mut chars = BTreeSet::new();
                        for spec in specs {
                            chars.extend(spec.to_char_set()?);
                        }
                        chars
                    }
                };
                Ok(full_alphabet.difference(&except_chars).cloned().collect())
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum YamlExceptValue {
    Single(YamlSymbolSpecifier),
    Multiple(Vec<YamlSymbolSpecifier>),
}

#[derive(Deserialize, Debug, Clone)]
struct YamlTransitionMapping {
    to: String,
    on: YamlTransitionOn,
}

pub fn from_yaml(yaml_content: &str) -> Result<DFA> {
    let yaml_dfa: YamlDFA = serde_yaml::from_str(yaml_content)?;

    let alphabet_set = read_alphabet(&yaml_dfa.alphabet)?;
    let alphabet_bimap: BiMap<char, usize> = alphabet_set
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, c)| (c, i))
        .collect();

    let state_keys: Vec<String> = yaml_dfa.states.keys().cloned().collect();
    let state_bimap: BiMap<String, usize> = state_keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, k)| (k, i))
        .collect();

    let state_props: Vec<YamlStateProps> = yaml_dfa.states.values().cloned().collect();
    let state_infos: Vec<StateInfo> = state_props
        .into_iter()
        .map(|p| StateInfo {
            label: p.label,
            accept: p.accept,
        })
        .collect();

    let start_state_index = get_state_idx(&state_bimap, &yaml_dfa.start_state)?;

    let transition_table = read_transitions(
        &state_bimap,
        yaml_dfa.transitions,
        &alphabet_set,
        &alphabet_bimap,
    )?;

    let accept_state_indices = state_bimap
        .iter()
        .filter_map(|(k, &v)| {
            if yaml_dfa.states.get(k).map_or(false, |props| props.accept) {
                Some(v)
            } else {
                None
            }
        })
        .collect();

    let dfa = DFA {
        name: yaml_dfa.name,
        description: yaml_dfa.description,
        alphabet: alphabet_bimap,
        state_keys: state_bimap,
        start_state_idx: start_state_index,
        accept_state_indices,
        transition_table,
        state_properties: state_infos,
    };

    Ok(dfa)
}

fn get_state_idx(state_bimap: &BiMap<String, usize>, state_key: &str) -> Result<usize> {
    state_bimap
        .get_by_left(state_key)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("State '{}' not found", state_key))
}

fn get_alphabet_idx(alphabet_bimap: &BiMap<char, usize>, c: char) -> Result<usize> {
    alphabet_bimap
        .get_by_left(&c)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Character '{}' not in alphabet (transition error)", c))
}

fn read_alphabet(yaml_alphabet: &[YamlSymbolSpecifier]) -> Result<BTreeSet<char>> {
    let mut alphabet = BTreeSet::new();

    for charset in yaml_alphabet {
        alphabet.extend(charset.to_char_set()?);
    }

    Ok(alphabet)
}

fn read_transitions(
    state_bimap: &BiMap<String, usize>,
    transitions: BTreeMap<String, Vec<YamlTransitionMapping>>,
    full_alphabet_set: &BTreeSet<char>,
    alphabet_bimap: &BiMap<char, usize>,
) -> Result<Vec<Vec<usize>>> {
    let state_count = state_bimap.len();
    let alphabet_size = alphabet_bimap.len();

    let mut transition_table = vec![vec![None; alphabet_size]; state_count];

    for (src_state_key, mappings) in transitions {
        let src_idx = get_state_idx(state_bimap, &src_state_key)?;

        for mapping in mappings {
            let dest_idx = get_state_idx(state_bimap, &mapping.to)?;

            let on_chars = mapping.on.to_char_set(full_alphabet_set)?;

            for c in on_chars {
                let alpha_idx = get_alphabet_idx(alphabet_bimap, c)?;

                match transition_table[src_idx][alpha_idx] {
                    Some(existing_dest_idx) => {
                        if existing_dest_idx != dest_idx {
                            // AMBIGUITY ERROR
                            let err_state = "ERR_STATE".to_string();
                            let existing_dest_key = state_bimap
                                .get_by_right(&existing_dest_idx)
                                .unwrap_or(&err_state);

                            return Err(anyhow::anyhow!(
                                "Ambiguous transition in state '{}' for symbol '{}': \
                                 maps to both '{}' and '{}'",
                                src_state_key,
                                c,
                                existing_dest_key,
                                mapping.to
                            ));
                        }
                    }
                    None => {
                        transition_table[src_idx][alpha_idx] = Some(dest_idx);
                    }
                }
            }
        }
    }

    let final_table = transition_table
        .into_iter()
        .enumerate()
        .map(|(src_idx, row)| {
            row.into_iter()
                .enumerate()
                .map(|(alpha_idx, dest_opt)| {
                    dest_opt.ok_or_else(|| {
                        // TOTALITY ERROR: This (state, symbol) pair was never defined.
                        let err_state = "ERR_STATE".to_string();
                        let src_key = state_bimap.get_by_right(&src_idx).unwrap_or(&err_state);
                        let symbol = alphabet_bimap.get_by_right(&alpha_idx).unwrap_or(&'?');

                        anyhow::anyhow!(
                            "Incomplete transitions for state '{}': \
                             no transition defined for symbol '{}'",
                            src_key,
                            symbol
                        )
                    })
                })
                .collect::<Result<Vec<usize>>>()
        })
        .collect::<Result<Vec<Vec<usize>>>>()?;

    Ok(final_table)
}
