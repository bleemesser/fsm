# YAML Specification for Defining Finite State Machines (FSM-YAML Spec)

**Version:** 1.1
**Date:** 2025-09-23

## 1. Introduction

This document specifies a human-readable YAML format for defining a Deterministic Finite Automaton (DFA). The goal is to provide an easy-to-write standard that is simple to parse.

A DFA is formally defined by a 5-tuple $(Q, \Sigma, \delta, q_0, F)$, where:

  - **$Q$** is a finite set of states.
  - **$\Sigma$** is a finite set of input symbols called the alphabet.
  - **$\delta$** is the transition function: $\delta: Q \times \Sigma \to Q$. This function must be total (defined for every state-symbol pair).
  - **$q_0$** is the start state.
  - **$F$** is the set of accept states.

An NFA extends this definition by allowing for any number of next states on a given input symbol (including 0), and for epsilon transitions on an empty string. Any NFA can be converted to an equivalent DFA via subset construction, which should be handled in the parsing step.

This specification maps each component of the FSM to a corresponding YAML structure.

## 2. Top-Level Structure

A FSM document is a YAML mapping that must contain the following top-level keys: `name`, `states`, `alphabet`, `start_state`, and `transitions`. Optional `description` and `dfa` keys are also supported.

```yaml
# Top-level structure
name: UniqueName
dfa: true # (Optional) Defaults to false. If true, enforce DFA rules.
description: # (Optional) A brief explanation of what this FSM does.
states: ...
alphabet: ...
start_state: ...
transitions: ...
```

## 3. Field Specifications

### 3.1. name

  - **Type:** String
  - **Cardinality:** Required, 1
  - **Description:** A unique identifier for the FSM.

### 3.2. dfa

  - **Type:** Boolean
  - **Cardinality:** Optional, 0..1
  - **Description:** Specifies the type of automaton.
      - If `true`, the parser will enforce strict DFA rules: transition totality and lack of ambiguity. The `epsilon` keyword in transitions is disallowed.
      - If `false` or omitted, the automaton is treated as an NFA. The parser will automatically convert it to an equivalent DFA using the subset construction algorithm.

### 3.3. description

  - **Type:** String
  - **Cardinality:** Optional, 0..1
  - **Description:** A human-readable description of the FSM's purpose.

### 3.4. states

  - **Type:** Mapping
  - **Cardinality:** Required, 1
  - **Description:** Defines the finite set of states ($Q$) and which of them are accept states ($F$).

  The keys of the `states` map are the friendly unique state identifiers. They can be any valid string, but should typically be more concise than the labels.

  The value for each state key is a nested map containing properties for that state.

#### State Properties:

  - **`accept`**: Boolean, Optional (defaults to `false`). If `true`, the state is an accepting state.
  - **`label`**: String, Optional. A human-readable label.

### 3.5. start_state

  - **Type:** String
  - **Cardinality:** Required, 1
  - **Description:** Defines the start state ($q_0$). Must be a key from the `states` map.

### 3.6. alphabet

  - **Type:** Sequence of Symbol Specifiers (see Appendix A)
  - **Cardinality:** Required, 1
  - **Description:** Defines the input alphabet ($\Sigma$). The final alphabet set is the union of all symbols derived from the specifiers in the sequence.

**Example:**

```yaml
alphabet:
  - { nrange: '0..9' } # Defines symbols 0, 1, ..., 9
  - '+'               # Defines the symbol +
  - '-'               # Defines the symbol -
```

### 3.7. transitions

  - **Type:** Mapping
  - **Cardinality:** Required, 1
  - **Description:** Defines the transition function ($\delta$) using a declarative, set-based rule system grouped by destination state.

The keys of the `transitions` map are the source state identifiers. The value for each source state is a Sequence of Transition Mappings.

**Validation:** A parser must validate two conditions for each source state:

1.  **No Ambiguity:** The symbol sets defined for each destination state must be disjoint. An input symbol cannot lead to more than one destination.
2.  **Totality:** The union of all symbol sets for all destinations must equal the entire alphabet ($\Sigma$).

#### 3.7.1. Transition Mapping Structure

Each item in the sequence is a mapping that groups all transitions from a source to a single destination. It must contain two keys: `to` and `on`.

  - **`to`**: Defines the destination state. The value must be a valid state identifier.
  - **`on`**: Defines the set of symbols that trigger this transition. The value can be:
      - A single Symbol Specifier (see Appendix A).
      - A Sequence of Symbol Specifiers. The resulting set is the union of all symbols defined.
      - The special keyword `alphabet`, representing all symbols in $\Sigma$.
      - The special keyword `epsilon`, for an epsilon transition (only allowed if `dfa` is `false`).
      - An `except` mapping, whose value can be a single Symbol Specifier or a Sequence of Symbol Specifiers. This matches all symbols in $\Sigma$ not in the set defined by the `except` value.

#### 3.7.2. Example

Consider a DFA that recognizes simple integers (e.g., `123`, `+45`, `-6`). The alphabet is defined as `[ {nrange: '0..9'}, '+', '-' ]`.

```yaml
transitions:
  q0: # Start state. Expecting a sign or a digit.
    - to: q1 # State for a leading sign
      on: ['+', '-']
    - to: q2 # State for digits (accepting)
      on: { nrange: '0..9' }

  q1: # Just saw a sign. Must be followed by a digit.
    - to: q2 # Go to the accepting digit state
      on: { nrange: '0..9' }
    - to: q3 # Anything else is invalid (dead state)
      on: ['+', '-'] # A sign cannot be followed by another sign

  q2: # In a valid number. Can see more digits.
    - to: q2 # Stay in the accepting state on more digits
      on: { nrange: '0..9' }
    - to: q3 # A sign after digits is invalid (dead state)
      on: ['+', '-']

  q3: # Dead state (trap)
    # On any symbol in the alphabet, stay in the dead state.
    - to: q3
      on: alphabet
```

## Appendix A: Symbol Set Notation

This specification uses a consistent notation to define sets of characters, both for the main `alphabet` and for `transitions`. The basic building block is the **Symbol Specifier**.

### Symbol Specifier Types

A Symbol Specifier can be one of the following:

  - **Literal**: A single character or number.
    *Example:* `'a'`, `_`, `5`

  - **String**: A string of characters. This is syntactic sugar for a sequence of its constituent character literals. A parser should treat `'abc'` as `['a', 'b', 'c']`.
    *Example:* `'xyz'`

  - **Range Mapping**: A mapping that defines an inclusive range of characters or numbers.

      - **`crange`**: An inclusive range of characters. *Example:* `{ crange: 'a..z' }`
      - **`nrange`**: An inclusive range of numeric characters. *Example:* `{ nrange: '0..9' }`