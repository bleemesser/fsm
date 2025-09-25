# FSM: YAML-Powered Finite State Machine Simulator

This project provides a tool for defining, loading, and running Finite Automata  from a simple YAML specification.

## Components

There are two primary components to this project:

1.  **FSM-YAML Specification**: A human-readable YAML format for defining a FSM's 5-tuple ($Q, \\Sigma, \\delta, q\_0, F$). It requires defining `name`, `states`, `alphabet`, `start_state`, and `transitions`. See `yaml_spec.md` for the complete specification.

2.  **CLI Tool (`fsm`)**: A command-line utility for interacting with FSM-YAML files. It can run in an interactive REPL mode to test strings or in a visualization mode to generate transition tables and Graphviz `.dot` files.

## Installation

Install the CLI tool from the project root using Cargo:

```sh
cargo install --path .
```

## Usage

The tool has two primary modes of operation.

### Interactive Mode

Run the REPL by providing the path to a YAML file. This mode allows you to test input strings against the loaded FSM.

```sh
fsm path/to/your/fsm.yml
```

Once loaded, you will be at a `>>` prompt.

  * **Test String**: Type any string and press Enter (e.g., `abab`) to see if the FSM accepts or rejects it.
  * **Commands**:
      * `load <file.yml>`: Load a new FSM.
      * `reload`: Reload the current FSM from its file.
      * `exit` / `quit`: Exit the REPL.

### Visualization Mode

Run the tool with the `--viz` flag to output the FSM's transition table to the console and generate a corresponding Graphviz `.dot` file.

```sh
fsm path/to/your/fsm.yml --viz
```

This will print the table and create a `.dot` file (e.g., `fsm.dot`) in the same directory, along with instructions for rendering it to an image.