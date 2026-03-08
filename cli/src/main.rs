mod report;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
struct DiagnoseArgs {
    #[command(subcommand)]
    target: DiagnoseTarget,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    output: OutputFormat,
    #[arg(long = "context-file", global = true)]
    context_file: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum DiagnoseTarget {
    Pod {
        name: String,
        #[arg(short = 'n', long = "namespace")]
        namespace: Option<String>,
    },
    Cluster {
        #[arg(short = 'n', long = "namespace")]
        namespace: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct PodDiagnosisOutput {
    pod: String,
    namespace: String,
    diagnoses: Vec<types::Diagnosis>,
    dependency_traces: Vec<engine::DependencyTrace>,
}

#[derive(Debug, Serialize)]
struct ClusterDiagnosisOutput {
    issue_count: usize,
    diagnoses: Vec<types::Diagnosis>,
    dependency_traces: Vec<engine::DependencyTrace>,
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
            DiagnoseTarget::Pod { name, namespace } => {
                diagnose_pod(name, namespace, args.output, args.context_file).await?
            }
            DiagnoseTarget::Cluster { namespace } => {
                diagnose_cluster(namespace, args.output, args.context_file).await?
            }
        },
    }

    Ok(())
}

async fn diagnose_pod(
    name: String,
    namespace: Option<String>,
    output: OutputFormat,
    context_file: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = if let Some(path) = context_file {
        load_context_from_file(&path)?
    } else if let Some(namespace) = namespace {
        cluster::collect_analysis_context_for_pod(&namespace, &name).await?
    } else {
        cluster::collect_analysis_context_for_current_namespace(&name).await?
    };
    let pod = ctx
        .pods
        .iter()
        .find(|pod| pod.name == name)
        .or_else(|| ctx.pods.first())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "collected context does not contain target pod",
            )
        })?;

    let engine = engine::Engine::new(
        analyzers::default_analyzers(),
        analyzers::default_graph_analyzers(),
    );
    let run = engine.run_report(&ctx);

    match output {
        OutputFormat::Text => report::print_pod_report(pod, run.diagnoses, run.dependency_traces),
        OutputFormat::Json => {
            let diagnoses = report::normalize_diagnoses(run.diagnoses);
            let payload = PodDiagnosisOutput {
                pod: pod.name.clone(),
                namespace: pod.namespace.clone(),
                diagnoses,
                dependency_traces: run.dependency_traces,
            };
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    }

    Ok(())
}

async fn diagnose_cluster(
    namespace: Option<String>,
    output: OutputFormat,
    context_file: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let run = if let Some(path) = context_file {
        let ctx = load_context_from_file(&path)?;
        let engine = engine::Engine::new(
            analyzers::default_analyzers(),
            analyzers::default_graph_analyzers(),
        );
        engine.run_report(&ctx)
    } else {
        let client = kube::Client::try_default().await?;
        if let Some(namespace) = namespace {
            engine::diagnose_report_in_namespace(client, &namespace).await?
        } else {
            engine::diagnose_report(client).await?
        }
    };

    match output {
        OutputFormat::Text => report::print_cluster_report(run.diagnoses, run.dependency_traces),
        OutputFormat::Json => {
            let diagnoses = report::normalize_diagnoses(run.diagnoses);
            let payload = ClusterDiagnosisOutput {
                issue_count: diagnoses.len(),
                diagnoses,
                dependency_traces: run.dependency_traces,
            };
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
    }

    Ok(())
}

fn load_context_from_file(path: &PathBuf) -> Result<types::AnalysisContext, Box<dyn std::error::Error>> {
    let input = std::fs::read_to_string(path)?;
    let context = serde_json::from_str::<types::AnalysisContext>(&input)?;
    Ok(context)
}
