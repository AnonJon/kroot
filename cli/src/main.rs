mod report;

use clap::{Parser, Subcommand};

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
            DiagnoseTarget::Pod { name, namespace } => diagnose_pod(name, namespace).await?,
            DiagnoseTarget::Cluster { namespace } => diagnose_cluster(namespace).await?,
        },
    }

    Ok(())
}

async fn diagnose_pod(
    name: String,
    namespace: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = if let Some(namespace) = namespace {
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

    let engine = engine::Engine::new(analyzers::default_analyzers());
    let diagnoses = engine.run(&ctx);
    let traces = engine::trace_missing_dependencies(pod);
    report::print_pod_report(pod, diagnoses, traces);

    Ok(())
}

async fn diagnose_cluster(namespace: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let client = kube::Client::try_default().await?;
    let diagnoses = if let Some(namespace) = namespace {
        engine::diagnose_in_namespace(client, &namespace).await?
    } else {
        engine::diagnose(client).await?
    };
    report::print_cluster_report(diagnoses);

    Ok(())
}
