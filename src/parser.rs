use crate::dfa::{Dfa, StateInfo};
use anyhow::{Result, anyhow};
use bimap::BiMap;
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

/// Intermediate representation of an NFA for subset construction.
#[derive(Debug, Clone)]
pub struct Nfa {
    /// Map from (from_state, on_char) to a set of destination states.
    /// `on_char = None` represents an epsilon transition.
    pub transitions: BTreeMap<(usize, Option<char>), BTreeSet<usize>>,
    pub start_state: usize,
    /// Set of original NFA state indices that are accepting.
    pub nfa_accept_states: BTreeSet<usize>,
    /// Original state keys, used for creating labels for new DFA states.
    pub nfa_state_keys: BiMap<String, usize>,
}

pub enum Fsm {
    Dfa(Dfa),
    Nfa { nfa: Nfa, dfa: Dfa },
}

impl Nfa {
    /// Creates an NFA from the parsed YAML components.
    fn from_yaml(
        state_bimap: &BiMap<String, usize>,
        start_state_idx: usize,
        state_infos: &[StateInfo],
        yaml_transitions: BTreeMap<String, Vec<YamlTransitionMapping>>,
        full_alphabet_set: &BTreeSet<char>,
    ) -> Result<Self> {
        let mut transitions = BTreeMap::new();
        let mut nfa_accept_states = BTreeSet::new();

        for (i, info) in state_infos.iter().enumerate() {
            if info.accept {
                nfa_accept_states.insert(i);
            }
        }

        for (src_key, mappings) in yaml_transitions {
            let src_idx = get_state_idx(state_bimap, &src_key)?;
            for mapping in mappings {
                let dest_idx = get_state_idx(state_bimap, &mapping.to)?;

                match mapping.on.to_transition_trigger(full_alphabet_set)? {
                    TransitionTrigger::Epsilon => {
                        transitions
                            .entry((src_idx, None))
                            .or_insert_with(BTreeSet::new)
                            .insert(dest_idx);
                    }
                    TransitionTrigger::Chars(chars) => {
                        for c in chars {
                            transitions
                                .entry((src_idx, Some(c)))
                                .or_insert_with(BTreeSet::new)
                                .insert(dest_idx);
                        }
                    }
                }
            }
        }

        Ok(Nfa {
            transitions,
            start_state: start_state_idx,
            nfa_accept_states,
            nfa_state_keys: state_bimap.clone(),
        })
    }

    /// Converts the NFA to an equivalent DFA using subset construction.
    fn to_dfa(
        self,
        name: &str,
        description: Option<String>,
        alphabet_set: &BTreeSet<char>,
    ) -> Result<Dfa> {
        let alphabet: Vec<char> = alphabet_set.iter().cloned().collect();
        let alphabet_bimap: BiMap<char, usize> = alphabet
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, c)| (c, i))
            .collect();

        // set of NFA states to new DFA state index
        let mut dfa_states: BTreeMap<BTreeSet<usize>, usize> = BTreeMap::new();
        let mut worklist: VecDeque<BTreeSet<usize>> = VecDeque::new();

        let mut dfa_state_keys = BiMap::new();
        let mut dfa_state_properties = Vec::new();
        let mut dfa_accept_states = Vec::new();
        let mut dfa_transitions = BTreeMap::new();

        let start_nfa_set = self.epsilon_closure(&BTreeSet::from([self.start_state]));

        let start_dfa_idx = 0;
        dfa_states.insert(start_nfa_set.clone(), start_dfa_idx);
        worklist.push_back(start_nfa_set);

        while let Some(current_nfa_set) = worklist.pop_front() {
            let current_dfa_idx = *dfa_states.get(&current_nfa_set).unwrap();

            for (alpha_idx, &symbol) in alphabet.iter().enumerate() {
                let directly_reachable_states = self.move_on_char(&current_nfa_set, symbol);
                let target_nfa_set = self.epsilon_closure(&directly_reachable_states);

                if target_nfa_set.is_empty() {
                    continue;
                }

                // add to DFA or get existing index
                let next_dfa_idx = if let Some(&idx) = dfa_states.get(&target_nfa_set) {
                    idx
                } else {
                    let new_idx = dfa_states.len();
                    dfa_states.insert(target_nfa_set.clone(), new_idx);
                    worklist.push_back(target_nfa_set);
                    new_idx
                };

                dfa_transitions.insert((current_dfa_idx, alpha_idx), next_dfa_idx);
            }
        }

        // dead state for missing transitions. equivalent to Ø state.
        let num_dfa_states = dfa_states.len();
        let mut needs_dead_state = false;
        'outer: for i in 0..num_dfa_states {
            for j in 0..alphabet.len() {
                if !dfa_transitions.contains_key(&(i, j)) {
                    needs_dead_state = true;
                    break 'outer;
                }
            }
        }

        let dead_state_idx = if needs_dead_state {
            let idx = num_dfa_states;
            for j in 0..alphabet.len() {
                dfa_transitions.insert((idx, j), idx);
            }
            Some(idx)
        } else {
            None
        };

        let total_dfa_states = num_dfa_states + if needs_dead_state { 1 } else { 0 };

        let mut sorted_dfa_states: Vec<(BTreeSet<usize>, usize)> = dfa_states.into_iter().collect();
        sorted_dfa_states.sort_by_key(|(_, idx)| *idx);

        for (nfa_set, dfa_idx) in sorted_dfa_states {
            let is_accepting = nfa_set
                .intersection(&self.nfa_accept_states)
                .next()
                .is_some();
            dfa_accept_states.push(is_accepting);

            let mut state_keys: Vec<&str> = nfa_set
                .iter()
                .map(|id| self.nfa_state_keys.get_by_right(id).unwrap().as_str())
                .collect();
            state_keys.sort();

            let new_key = format!("{{{}}}", state_keys.join(","));
            dfa_state_keys.insert(new_key.clone(), dfa_idx);
            dfa_state_properties.push(StateInfo {
                label: Some(new_key),
                accept: is_accepting,
            });
        }

        if let Some(idx) = dead_state_idx {
            let key = "FAILURE".to_string();
            dfa_state_keys.insert(key.clone(), idx);
            dfa_state_properties.push(StateInfo {
                label: Some(key),
                accept: false,
            });
            dfa_accept_states.push(false);
        }

        let mut transition_table =
            vec![dead_state_idx.unwrap_or(0); total_dfa_states * alphabet.len()];
        for ((from, alpha), to) in dfa_transitions {
            transition_table[from * alphabet.len() + alpha] = to;
        }

        Ok(Dfa {
            name: name.to_string(),
            description,
            alphabet: alphabet_bimap,
            state_keys: dfa_state_keys,
            start_state_idx: start_dfa_idx,
            accept_states: dfa_accept_states,
            transition_table,
            state_properties: dfa_state_properties,
        })
    }

    /// Calculates the epsilon closure for a given set of NFA states.
    fn epsilon_closure(&self, states: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut closure = states.clone();
        let mut worklist: Vec<usize> = states.iter().cloned().collect();
        while let Some(state) = worklist.pop() {
            if let Some(epsilon_dests) = self.transitions.get(&(state, None)) {
                for &dest in epsilon_dests {
                    if closure.insert(dest) {
                        worklist.push(dest);
                    }
                }
            }
        }
        closure
    }

    /// Finds all states reachable from a set of states on a given character.
    fn move_on_char(&self, states: &BTreeSet<usize>, symbol: char) -> BTreeSet<usize> {
        let mut result = BTreeSet::new();
        for &state in states {
            if let Some(dests) = self.transitions.get(&(state, Some(symbol))) {
                result.extend(dests);
            }
        }
        result
    }
}

#[derive(Deserialize, Debug)]
struct YamlDFA {
    name: String,
    #[serde(default)]
    dfa: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum YamlSymbolSpecifier {
    Literal(String),
    Map(YamlRangeMap),
}

// for some reason the default Deserialize implementation doesn't work here
// so i'm using a manual implementation. the exact problem was that except + a list was being
// interpreted as a range map with a start but no end, which was erroring.
impl<'de> Deserialize<'de> for YamlSymbolSpecifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct YamlSymbolSpecifierVisitor;

        impl<'de> Visitor<'de> for YamlSymbolSpecifierVisitor {
            type Value = YamlSymbolSpecifier;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a string literal (e.g., 'a') or a range map (e.g., { crange: 'a..z' })",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(YamlSymbolSpecifier::Literal(value.to_string()))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let range_map =
                    YamlRangeMap::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(YamlSymbolSpecifier::Map(range_map))
            }
        }

        deserializer.deserialize_any(YamlSymbolSpecifierVisitor)
    }
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
                        return Err(anyhow!("Invalid character range: {}", crange));
                    }
                    let start = parts[0]
                        .chars()
                        .next()
                        .ok_or_else(|| anyhow!("Empty start in range: {}", crange))?;
                    let end = parts[1]
                        .chars()
                        .next()
                        .ok_or_else(|| anyhow!("Empty end in range: {}", crange))?;
                    if start > end {
                        return Err(anyhow!(
                            "Start character greater than end in range: {}",
                            crange
                        ));
                    }
                    for c in (start as u32)..=(end as u32) {
                        char_set.insert(std::char::from_u32(c).unwrap_or_else(|| {
                            panic!("Invalid character in range: {}..{}", start, end)
                        }));
                    }
                }
                if let Some(nrange) = &range_map.nrange {
                    let parts: Vec<&str> = nrange.split("..").collect();
                    if parts.len() != 2 {
                        return Err(anyhow!("Invalid numeric range: {}", nrange));
                    }
                    let start: u8 = parts[0]
                        .parse()
                        .map_err(|_| anyhow!("Invalid start number in range: {}", nrange))?;
                    let end: u8 = parts[1]
                        .parse()
                        .map_err(|_| anyhow!("Invalid end number in range: {}", nrange))?;
                    if start > end {
                        return Err(anyhow!(
                            "Start number greater than end in range: {}",
                            nrange
                        ));
                    }
                    if start > 9 || end > 9 {
                        return Err(anyhow!("Numeric range must be between 0 and 9: {}", nrange));
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
    #[serde(rename = "epsilon")]
    Epsilon,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum YamlTransitionOn {
    Except { except: YamlExceptValue },
    Keyword(YamlKeyword),
    Single(YamlSymbolSpecifier),
    Multiple(Vec<YamlSymbolSpecifier>),
}

enum TransitionTrigger {
    Chars(BTreeSet<char>),
    Epsilon,
}

impl YamlTransitionOn {
    fn to_transition_trigger(&self, full_alphabet: &BTreeSet<char>) -> Result<TransitionTrigger> {
        match self {
            YamlTransitionOn::Single(spec) => Ok(TransitionTrigger::Chars(spec.to_char_set()?)),
            YamlTransitionOn::Multiple(specs) => {
                let mut char_set = BTreeSet::new();
                for spec in specs {
                    char_set.extend(spec.to_char_set()?);
                }
                Ok(TransitionTrigger::Chars(char_set))
            }
            YamlTransitionOn::Keyword(kw) => match kw {
                YamlKeyword::Alphabet => Ok(TransitionTrigger::Chars(full_alphabet.clone())),
                YamlKeyword::Epsilon => Ok(TransitionTrigger::Epsilon),
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
                Ok(TransitionTrigger::Chars(
                    full_alphabet.difference(&except_chars).cloned().collect(),
                ))
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

pub fn from_yaml(yaml_content: &str) -> Result<Fsm> {
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

    if yaml_dfa.dfa {
        let transition_table = build_dfa_transitions(
            &state_bimap,
            yaml_dfa.transitions,
            &alphabet_set,
            &alphabet_bimap,
        )?;
        let accept_states = state_infos.iter().map(|info| info.accept).collect();
        Ok(Fsm::Dfa(Dfa {
            name: yaml_dfa.name,
            description: yaml_dfa.description,
            alphabet: alphabet_bimap,
            state_keys: state_bimap,
            start_state_idx: start_state_index,
            accept_states,
            transition_table,
            state_properties: state_infos,
        }))
    } else {
        let nfa = Nfa::from_yaml(
            &state_bimap,
            start_state_index,
            &state_infos,
            yaml_dfa.transitions,
            &alphabet_set,
        )?;
        let dfa = nfa
            .clone()
            .to_dfa(&yaml_dfa.name, yaml_dfa.description, &alphabet_set)?;
        Ok(Fsm::Nfa { nfa, dfa })
    }
}

fn get_state_idx(state_bimap: &BiMap<String, usize>, state_key: &str) -> Result<usize> {
    state_bimap
        .get_by_left(state_key)
        .cloned()
        .ok_or_else(|| anyhow!("State '{}' not found", state_key))
}

fn get_alphabet_idx(alphabet_bimap: &BiMap<char, usize>, c: char) -> Result<usize> {
    alphabet_bimap
        .get_by_left(&c)
        .cloned()
        .ok_or_else(|| anyhow!("Character '{}' not in alphabet (transition error)", c))
}

fn read_alphabet(yaml_alphabet: &[YamlSymbolSpecifier]) -> Result<BTreeSet<char>> {
    let mut alphabet = BTreeSet::new();

    for charset in yaml_alphabet {
        alphabet.extend(charset.to_char_set()?);
    }

    Ok(alphabet)
}

fn build_dfa_transitions(
    state_bimap: &BiMap<String, usize>,
    transitions: BTreeMap<String, Vec<YamlTransitionMapping>>,
    full_alphabet_set: &BTreeSet<char>,
    alphabet_bimap: &BiMap<char, usize>,
) -> Result<Vec<usize>> {
    let state_count = state_bimap.len();
    let alphabet_size = alphabet_bimap.len();

    let mut transition_table = vec![None; state_count * alphabet_size];

    for (src_state_key, mappings) in transitions {
        let src_idx = get_state_idx(state_bimap, &src_state_key)?;

        for mapping in mappings {
            let dest_idx = get_state_idx(state_bimap, &mapping.to)?;

            // let on_chars = mapping.on.to_transition_trigger(full_alphabet_set)?;
            match mapping.on.to_transition_trigger(full_alphabet_set)? {
                TransitionTrigger::Epsilon => {
                    return Err(anyhow!(
                        "Epsilon transitions are not allowed when 'dfa' flag is true. (state '{}')",
                        src_state_key
                    ));
                }
                TransitionTrigger::Chars(on_chars) => {
                    for c in on_chars {
                        let alpha_idx = get_alphabet_idx(alphabet_bimap, c)?;

                        let table_idx = src_idx * alphabet_size + alpha_idx;

                        match transition_table[table_idx] {
                            Some(existing_dest_idx) => {
                                if existing_dest_idx != dest_idx {
                                    // AMBIGUITY ERROR: Two different transitions exist for same (state, symbol) pair.
                                    let err_state = "ERR_STATE".to_string();
                                    let existing_dest_key = state_bimap
                                        .get_by_right(&existing_dest_idx)
                                        .unwrap_or(&err_state);

                                    return Err(anyhow!(
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
                                transition_table[table_idx] = Some(dest_idx);
                            }
                        }
                    }
                }
            }
        }
    }

    let final_table = transition_table
        .into_iter()
        .enumerate()
        .map(|(table_idx, dest_opt)| {
            dest_opt.ok_or_else(|| {
                // TOTALITY ERROR: This (state, symbol) pair was never defined.
                let src_idx = table_idx / alphabet_size;
                let alpha_idx = table_idx % alphabet_size;

                let err_state = "ERR_STATE".to_string();
                let src_key = state_bimap.get_by_right(&src_idx).unwrap_or(&err_state);
                let symbol = alphabet_bimap.get_by_right(&alpha_idx).unwrap_or(&'?');

                anyhow!(
                    "Incomplete transitions for state '{}': \
                     no transition defined for symbol '{}'",
                    src_key,
                    symbol
                )
            })
        })
        .collect::<Result<Vec<usize>>>()?;

    Ok(final_table)
}
