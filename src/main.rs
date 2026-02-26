use std::{env, fs};

use forgequeue::{select_batch_detailed_filtered, BatchPlan, Task};

#[derive(Debug)]
struct CliOptions {
    input: Option<String>,
    budget: f64,
    min_score: f64,
    top: usize,
    output_json: bool,
}

fn print_usage() {
    println!("ForgeQueue RS");
    println!("Usage:");
    println!("  cargo run -- [--input <json-file>] [--budget <value>] [--min-score <value>] [--top <n>] [--json]");
    println!();
    println!("Example:");
    println!("  cargo run -- --input data/tasks.sample.json --budget 7 --json");
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut input = None;
    let mut budget = 7.0;
    let mut min_score = 0.0;
    let mut top = 0usize;
    let mut output_json = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--input" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| String::from("Missing value for --input"))?;
                input = Some(value.clone());
            }
            "--budget" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| String::from("Missing value for --budget"))?;
                budget = value
                    .parse::<f64>()
                    .map_err(|_| String::from("--budget must be a number"))?;
            }
            "--json" => {
                output_json = true;
            }
            "--min-score" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| String::from("Missing value for --min-score"))?;
                min_score = value
                    .parse::<f64>()
                    .map_err(|_| String::from("--min-score must be a number"))?;
            }
            "--top" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| String::from("Missing value for --top"))?;
                top = value
                    .parse::<usize>()
                    .map_err(|_| String::from("--top must be a positive integer"))?;
            }
            unknown => {
                return Err(format!("Unknown argument: {unknown}"));
            }
        }
        index += 1;
    }

    Ok(CliOptions {
        input,
        budget,
        min_score,
        top,
        output_json,
    })
}

fn default_tasks() -> Vec<Task> {
    vec![
        Task {
            name: "Portfolio performance pass".to_string(),
            business_value: 9.0,
            time_criticality: 8.0,
            risk_reduction: 6.0,
            effort: 4.0,
        },
        Task {
            name: "CLI telemetry extension".to_string(),
            business_value: 8.0,
            time_criticality: 7.0,
            risk_reduction: 7.0,
            effort: 3.5,
        },
        Task {
            name: "UI polish sweep".to_string(),
            business_value: 6.5,
            time_criticality: 5.0,
            risk_reduction: 4.0,
            effort: 2.5,
        },
    ]
}

fn parse_tasks_json(raw: &str) -> Result<Vec<Task>, String> {
    let tasks: Vec<Task> = serde_json::from_str(raw).map_err(|err| format!("Invalid JSON: {err}"))?;
    if tasks.is_empty() {
        return Err(String::from("Input file must include at least one task."));
    }
    Ok(tasks)
}

fn load_tasks(input_path: &str) -> Result<Vec<Task>, String> {
    let raw = fs::read_to_string(input_path)
        .map_err(|err| format!("Unable to read input file '{input_path}': {err}"))?;
    parse_tasks_json(&raw)
}

fn print_text_plan(plan: &BatchPlan) {
    println!(
        "Selected batch (budget {:.2}, used {:.2}, skipped {}):",
        plan.budget, plan.used_effort, plan.skipped_tasks
    );
    for item in &plan.selected {
        println!(
            " - {} | score {:.2} | effort {:.2}",
            item.task.name, item.score, item.task.effort
        );
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = parse_args(&args)?;
    let tasks = match options.input {
        Some(input_path) => load_tasks(&input_path)?,
        None => default_tasks(),
    };

    let plan = select_batch_detailed_filtered(&tasks, options.budget, options.min_score, options.top);

    if options.output_json {
        let payload = serde_json::to_string_pretty(&plan).map_err(|err| format!("Serialization error: {err}"))?;
        println!("{payload}");
    } else {
        print_text_plan(&plan);
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        eprintln!();
        print_usage();
        std::process::exit(1);
    }
}
