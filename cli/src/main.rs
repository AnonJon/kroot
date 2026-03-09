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
    Sarif,
}

#[derive(Parser, Debug)]
struct DiagnoseArgs {
    #[command(subcommand)]
    target: DiagnoseTarget,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    output: OutputFormat,
    #[arg(long = "context-file", global = true)]
    context_file: Option<PathBuf>,
    #[arg(short = 'A', long = "all-namespaces", global = true)]
    all_namespaces: bool,
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

#[derive(Debug, Serialize)]
struct SarifLog {
    version: String,
    #[serde(rename = "$schema")]
    schema: String,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Debug, Serialize)]
struct SarifDriver {
    name: String,
    information_uri: String,
    rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize)]
struct SarifRule {
    id: String,
    name: String,
    short_description: SarifMessage,
}

#[derive(Debug, Serialize)]
struct SarifResult {
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
    properties: SarifProperties,
}

#[derive(Debug, Serialize)]
struct SarifLocation {
    logical_locations: Vec<SarifLogicalLocation>,
}

#[derive(Debug, Serialize)]
struct SarifLogicalLocation {
    name: String,
}

#[derive(Debug, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Serialize)]
struct SarifProperties {
    confidence: f32,
    root_cause: String,
    evidence: Vec<String>,
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
                diagnose_pod(
                    name,
                    namespace,
                    args.output,
                    args.context_file,
                    args.all_namespaces,
                )
                .await?
            }
            DiagnoseTarget::Cluster { namespace } => {
                diagnose_cluster(
                    namespace,
                    args.output,
                    args.context_file,
                    args.all_namespaces,
                )
                .await?
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
    all_namespaces: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if all_namespaces {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--all-namespaces is only supported with `diagnose cluster`",
        )
        .into());
    }

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
        OutputFormat::Sarif => {
            let diagnoses = report::normalize_diagnoses(run.diagnoses);
            println!(
                "{}",
                serde_json::to_string_pretty(&build_sarif_log(&diagnoses))?
            );
        }
    }

    Ok(())
}

async fn diagnose_cluster(
    namespace: Option<String>,
    output: OutputFormat,
    context_file: Option<PathBuf>,
    all_namespaces: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if all_namespaces && namespace.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--all-namespaces cannot be combined with --namespace",
        )
        .into());
    }

    let run = if let Some(path) = context_file {
        let ctx = load_context_from_file(&path)?;
        let engine = engine::Engine::new(
            analyzers::default_analyzers(),
            analyzers::default_graph_analyzers(),
        );
        engine.run_report(&ctx)
    } else {
        let client = kube::Client::try_default().await?;
        if all_namespaces {
            engine::diagnose_report_all_namespaces(client).await?
        } else if let Some(namespace) = namespace {
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
        OutputFormat::Sarif => {
            let diagnoses = report::normalize_diagnoses(run.diagnoses);
            println!(
                "{}",
                serde_json::to_string_pretty(&build_sarif_log(&diagnoses))?
            );
        }
    }

    Ok(())
}

fn load_context_from_file(
    path: &PathBuf,
) -> Result<types::AnalysisContext, Box<dyn std::error::Error>> {
    let input = std::fs::read_to_string(path)?;
    let context = serde_json::from_str::<types::AnalysisContext>(&input)?;
    Ok(context)
}

fn build_sarif_log(diagnoses: &[types::Diagnosis]) -> SarifLog {
    let mut rules = std::collections::BTreeMap::new();
    let mut results = Vec::new();

    for diagnosis in diagnoses {
        let rule_id = diagnosis.message.replace(' ', "_").to_lowercase();
        rules.entry(rule_id.clone()).or_insert_with(|| SarifRule {
            id: rule_id.clone(),
            name: diagnosis.message.clone(),
            short_description: SarifMessage {
                text: diagnosis.root_cause.clone(),
            },
        });

        results.push(SarifResult {
            rule_id,
            level: sarif_level(diagnosis.severity).to_string(),
            message: SarifMessage {
                text: diagnosis.message.clone(),
            },
            locations: vec![SarifLocation {
                logical_locations: vec![SarifLogicalLocation {
                    name: diagnosis.resource.clone(),
                }],
            }],
            properties: SarifProperties {
                confidence: diagnosis.confidence,
                root_cause: diagnosis.root_cause.clone(),
                evidence: diagnosis.evidence.clone(),
            },
        });
    }

    SarifLog {
        version: "2.1.0".to_string(),
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "kdocter".to_string(),
                    information_uri: "https://github.com/AnonJon/kdocter".to_string(),
                    rules: rules.into_values().collect(),
                },
            },
            results,
        }],
    }
}

fn sarif_level(severity: types::Severity) -> &'static str {
    match severity {
        types::Severity::Critical => "error",
        types::Severity::Warning => "warning",
        types::Severity::Info => "note",
    }
}
