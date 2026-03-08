use clap::{Parser, Subcommand};
use colored::Colorize;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
#[command(name = "kdocter", about = "Kubernetes root cause analysis CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Diagnose(DiagnoseArgs),
}

#[derive(Parser, Debug)]
struct DiagnoseArgs {
    #[command(subcommand)]
    target: DiagnoseTarget,
}

#[derive(Subcommand, Debug)]
enum DiagnoseTarget {
    Pod { name: String },
}

#[derive(Tabled)]
struct EvidenceRow {
    diagnosis: String,
    item: String,
}

#[derive(Tabled)]
struct DiagnosisRow {
    severity: String,
    status: String,
    root_cause: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Diagnose(args) => match args.target {
            DiagnoseTarget::Pod { name } => diagnose_pod(name).await?,
        },
    }

    Ok(())
}

async fn diagnose_pod(name: String) -> Result<(), Box<dyn std::error::Error>> {
    let pod = cluster::fetch_pod_state(&name).await?;
    let ctx = types::AnalysisContext { pod: pod.clone() };
    let engine = engine::Engine::new(analyzers::default_analyzers());
    let diagnoses = engine.run(&ctx);

    println!("{}", "Diagnosis Report".bold().blue());
    println!("{}", "----------------".blue());
    println!();
    println!("Pod: {}", pod.name.bold());
    println!("Namespace: {}", pod.namespace.bold());
    println!();

    let status = if diagnoses.is_empty() {
        pod.phase.as_str().to_string()
    } else {
        "Issues detected".to_string()
    };
    println!("Status: {}", status.yellow().bold());
    println!();

    println!("{}", "Diagnoses:".bold());
    let diagnosis_rows = if diagnoses.is_empty() {
        vec![DiagnosisRow {
            severity: "INFO".to_string(),
            status: "No diagnosis".to_string(),
            root_cause: "No issue detected".to_string(),
        }]
    } else {
        diagnoses
            .iter()
            .map(|diag| DiagnosisRow {
                severity: match diag.severity {
                    types::Severity::Info => "INFO".to_string(),
                    types::Severity::Warning => "WARNING".to_string(),
                    types::Severity::Critical => "CRITICAL".to_string(),
                },
                status: diag.message.clone(),
                root_cause: diag.root_cause.clone(),
            })
            .collect::<Vec<_>>()
    };
    println!("{}", Table::new(diagnosis_rows));

    println!("{}", "Evidence:".bold());
    let evidence_rows = if diagnoses.is_empty() {
        vec![EvidenceRow {
            diagnosis: "None".to_string(),
            item: format!("Pod phase: {}", pod.phase),
        }]
    } else {
        diagnoses
            .iter()
            .flat_map(|diag| {
                if diag.evidence.is_empty() {
                    vec![EvidenceRow {
                        diagnosis: diag.message.clone(),
                        item: "No evidence captured".to_string(),
                    }]
                } else {
                    diag.evidence
                        .iter()
                        .map(|item| EvidenceRow {
                            diagnosis: diag.message.clone(),
                            item: item.clone(),
                        })
                        .collect::<Vec<_>>()
                }
            })
            .collect::<Vec<_>>()
    };
    println!("{}", Table::new(evidence_rows));

    Ok(())
}
