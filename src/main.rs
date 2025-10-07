use anyhow::Result;
use clap::Parser;
use fsm::parser::Fsm;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A command-line tool for loading and running Deterministic Finite Automata (DFA)
/// from YAML specifications.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The path to the .yml DFA specification file.
    #[arg(required = true)]
    file: PathBuf,

    /// Generate a Graphviz DOT file for visualization.
    #[arg(long)]
    viz: bool,

    /// Print the transition table to the console.
    #[arg(long)]
    table: bool,
}

fn main() {
    if let Err(e) = run_cli() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// The main CLI logic, handling argument parsing, FSM loading, and REPL.
fn run_cli() -> Result<()> {
    let args = Args::parse();

    let mut fsm = load_fsm(&args.file)?;
    let mut current_path = args.file.clone();

    if args.table {
        match &fsm {
            Fsm::Dfa(dfa) => dfa.print_transition_table(),
            Fsm::Nfa { dfa, .. } => dfa.print_transition_table(),
        }
    } else if args.viz {
        run_viz(&fsm, &current_path)?;
    } else {
        println!(
            "Loading DFA with {} states and {} transitions...",
            match &fsm {
                Fsm::Dfa(dfa) => dfa.state_keys.len(),
                Fsm::Nfa { dfa, .. } => dfa.state_keys.len(),
            },
            match &fsm {
                Fsm::Dfa(dfa) => dfa.transition_table.len(),
                Fsm::Nfa { dfa, .. } => dfa.transition_table.len(),
            },
        );
        println!(
            "FSM '{}' loaded. (Press Ctrl+C or type 'exit' to quit)",
            match &fsm {
                Fsm::Dfa(dfa) => &dfa.name,
                Fsm::Nfa { dfa, .. } => &dfa.name,
            }
        );
        println!("Commands: 'exit', 'reload', 'load <file.yml>'");

        let mut rl = Editor::<(), FileHistory>::new()?;

        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    let input = line.trim();
                    if !input.is_empty() {
                        rl.add_history_entry(input)?;
                    }

                    match input {
                        "exit" | "quit" => break,
                        "reload" => {
                            println!("Reloading '{}'...", current_path.display());
                            match load_fsm(&current_path) {
                                Ok(new_fsm) => {
                                    fsm = new_fsm;
                                    println!(
                                        "FSM '{}' reloaded successfully.",
                                        match &fsm {
                                            Fsm::Dfa(dfa) => &dfa.name,
                                            Fsm::Nfa { dfa, .. } => &dfa.name,
                                        }
                                    );
                                }
                                Err(e) => eprintln!("Failed to reload: {}", e),
                            }
                        }
                        _ if input.starts_with("load ") => {
                            if let Some(path_str) = input.strip_prefix("load ").map(str::trim) {
                                let new_path = PathBuf::from(path_str);
                                println!("Loading '{}'...", new_path.display());
                                match load_fsm(&new_path) {
                                    Ok(new_fsm) => {
                                        fsm = new_fsm;
                                        current_path = new_path;
                                        println!(
                                            "FSM '{}' loaded successfully.",
                                            match &fsm {
                                                Fsm::Dfa(dfa) => &dfa.name,
                                                Fsm::Nfa { dfa, .. } => &dfa.name,
                                            }
                                        );
                                    }
                                    Err(e) => eprintln!("Failed to load: {}", e),
                                }
                            } else {
                                eprintln!("Invalid load command. Use: load <file.yml>");
                            }
                        }
                        _ => {
                            let dfa = match &fsm {
                                Fsm::Dfa(dfa) => dfa,
                                Fsm::Nfa { dfa, .. } => dfa,
                            };
                            let start_time = std::time::Instant::now();
                            let accepted = dfa.run(input.chars());
                            let duration = start_time.elapsed();
                            println!(
                                "{} | Processed in: {:.2?}",
                                if accepted { "ACCEPT" } else { "REJECT" },
                                duration
                            );
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C
                    println!("Exiting.");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl-D
                    println!("Exiting.");
                    break;
                }
                Err(err) => {
                    eprintln!("REPL Error: {:?}", err);
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Helper function to load a FSM from a file path.
fn load_fsm(path: &Path) -> Result<Fsm> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let fsm = fsm::parser::from_yaml(&contents)?;

    Ok(fsm)
}

/// Helper function to run the visualization logic.
fn run_viz(fsm: &Fsm, file_path: &Path) -> Result<()> {
    match fsm {
        Fsm::Dfa(dfa) => {
            let dot_filename = file_path.with_extension("dot");
            fsm::dot_generator::make_dot(dfa, &dot_filename)?;
            generate_and_print_viz_instructions(file_path, "")?;
        }
        Fsm::Nfa { nfa, dfa } => {
            // NFA visualization
            let nfa_dot_filename = file_path.with_file_name(format!(
                "{}-nfa.dot",
                file_path.file_stem().unwrap().to_str().unwrap()
            ));
            fsm::dot_generator::make_nfa_dot(
                nfa,
                &dfa.name,
                dfa.description.as_deref(),
                &nfa_dot_filename,
            )?;
            generate_and_print_viz_instructions(file_path, "-nfa")?;

            // DFA visualization
            let dfa_dot_filename = file_path.with_file_name(format!(
                "{}-dfa.dot",
                file_path.file_stem().unwrap().to_str().unwrap()
            ));
            fsm::dot_generator::make_dot(dfa, &dfa_dot_filename)?;
            generate_and_print_viz_instructions(file_path, "-dfa")?;
        }
    }

    Ok(())
}

/// Helper function to print instructions for generating visualizations.
fn generate_and_print_viz_instructions(file_path: &Path, stem_suffix: &str) -> Result<()> {
    let dot_filename = file_path.with_file_name(format!(
        "{}{}.dot",
        file_path.file_stem().unwrap().to_str().unwrap(),
        stem_suffix
    ));
    let dot_str = dot_filename.to_str().unwrap_or("fsm.dot");

    println!("\nGraphviz DOT file generated: {}", dot_str);

    println!("\nTo generate a PNG, use Graphviz:");
    println!(
        "  dot -Tpng \"{}\" -o \"{}\"",
        dot_str,
        file_path
            .with_file_name(format!(
                "{}{}.png",
                file_path.file_stem().unwrap().to_str().unwrap(),
                stem_suffix
            ))
            .to_str()
            .unwrap_or("fsm.png")
    );

    Ok(())
}
