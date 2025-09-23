use anyhow::Result;
use clap::Parser;
use fsm::dfa::DFA;
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

fn run_cli() -> Result<()> {
    let args = Args::parse();

    let mut dfa = load_dfa(&args.file)?;
    let mut current_path = args.file.clone();

    if args.table {
        dfa.print_transition_table();
    } else if args.viz {
        run_viz(&dfa, &current_path)?;
    } else {
        println!(
            "DFA '{}' loaded. (Press Ctrl+C or type 'exit' to quit)",
            dfa.name
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
                            match load_dfa(&current_path) {
                                Ok(new_dfa) => {
                                    dfa = new_dfa;
                                    println!("DFA '{}' reloaded successfully.", dfa.name);
                                }
                                Err(e) => eprintln!("Failed to reload: {}", e),
                            }
                        }
                        _ if input.starts_with("load ") => {
                            if let Some(path_str) = input.strip_prefix("load ").map(str::trim) {
                                let new_path = PathBuf::from(path_str);
                                println!("Loading '{}'...", new_path.display());
                                match load_dfa(&new_path) {
                                    Ok(new_dfa) => {
                                        dfa = new_dfa;
                                        current_path = new_path;
                                        println!("DFA '{}' loaded successfully.", dfa.name);
                                    }
                                    Err(e) => eprintln!("Failed to load: {}", e),
                                }
                            } else {
                                eprintln!("Invalid load command. Use: load <file.yml>");
                            }
                        }
                        _ => {
                            let start_time = std::time::Instant::now();
                            let accepted = dfa.run(input.chars());
                            let duration = start_time.elapsed();
                            println!("{} | Processed in: {:.2?}", if accepted { "ACCEPT" } else { "REJECT" }, duration);
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

/// Helper function to load a DFA from a file path.
fn load_dfa(path: &Path) -> Result<DFA> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let dfa = DFA::from_yaml(&contents)?;

    // let runs = 10;
    // let mut total_time_ns = 0u128;

    // let correct_value = true;
    // let test_input = "a".chars().cycle().take(1_000_000_000);
    
    // for i in 0..runs {
    //     let input = test_input.clone();
    //     let start_time = std::time::Instant::now();
        
    //     let accepted = dfa.run(input);
    //     let duration = start_time.elapsed();

    //     if accepted != correct_value {
    //         println!(
    //             "Warning: Test input returned {}, expected {}",
    //             if accepted { "ACCEPT" } else { "REJECT" },
    //             if correct_value { "ACCEPT" } else { "REJECT" }
    //         );
    //     }
    //     println!("Run {} of {} completed in: {:.2?}", i + 1, runs, duration);
    //     total_time_ns += duration.as_nanos();
    // }
    // let avg_time_ns = total_time_ns as f64 / runs as f64;

    // println!(
    //     "Benchmark: {} (Average over {} runs: {:.2} ms)",
    //     if dfa.run(test_input) { "ACCEPT" } else { "REJECT" },
    //     runs,
    //     avg_time_ns / 1_000_000.0
    // );

    Ok(dfa)
}

/// Helper function to run the visualization logic.
fn run_viz(dfa: &DFA, file_path: &Path) -> Result<()> {
    let dot_filename = file_path.with_extension("dot");
    let dot_str = dot_filename.to_str().unwrap_or("dfa.dot");

    fsm::dot_generator::make_dot(dfa, &dot_filename)?;
    println!("\nGraphviz DOT file generated: {}", dot_str);

    println!("\nTo generate a PNG, use Graphviz:");
    println!(
        "  dot -Tpng \"{}\" -o \"{}\"",
        dot_str,
        file_path
            .with_extension("png")
            .to_str()
            .unwrap_or("dfa.png")
    );

    Ok(())
}
