use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use burst_frame_deduplicator::artifacts;
use burst_frame_deduplicator::counterpart::{
    apply_counterparts, plan_counterparts, restore_counterparts,
};
use burst_frame_deduplicator::pipeline::run_scan_controlled;
use burst_frame_deduplicator::progress::{
    CancellationToken, is_scan_cancelled, terminal_progress_reporter,
};
use burst_frame_deduplicator::run_storage;
use burst_frame_deduplicator::server;
use burst_frame_deduplicator::types::{
    AccelerationPreference, DetectorDevicePreference, DetectorModelPreference, DetectorPreference,
    ScanOptions,
};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Generic burst-frame deduplicator for RAW/JPEG photo bursts"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scan a folder or mounted SD card and write review artifacts.
    Scan {
        /// Source folder, for example /Volumes/CARD/DCIM.
        root: PathBuf,
        /// Output run directory. Defaults to a timestamped folder under ./runs.
        #[arg(long)]
        out: Option<PathBuf>,
        #[command(flatten)]
        options: ScanArgs,
    },
    /// Scan and then serve the local review UI.
    App {
        /// Source folder, for example /Volumes/CARD/DCIM.
        root: PathBuf,
        /// Output run directory. Defaults to a timestamped folder under ./runs.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 7878)]
        port: u16,
        #[arg(long)]
        open: bool,
        #[command(flatten)]
        options: ScanArgs,
    },
    /// Serve the local review UI for an existing run directory.
    Serve {
        /// Existing run directory containing manifest.json.
        #[arg(long)]
        run: PathBuf,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 7878)]
        port: u16,
        #[arg(long)]
        open: bool,
    },
    /// Re-export keep/reject CSVs and move script after review decisions changed.
    Export {
        /// Existing run directory containing manifest.json and review_state.json.
        #[arg(long)]
        run: PathBuf,
    },
    /// Move a completed run folder to a different result directory.
    Relocate {
        /// Existing run directory containing manifest.json.
        #[arg(long)]
        run: PathBuf,
        /// Parent directory that should contain the relocated run folder.
        #[arg(long)]
        to: PathBuf,
    },
    /// Preview basename-only matches on a swapped RAW/JPEG card without changing files.
    CounterpartPlan {
        /// Existing run directory containing reviewed decisions.
        #[arg(long)]
        run: PathBuf,
        /// Root folder of the currently mounted counterpart card.
        #[arg(long)]
        card: PathBuf,
    },
    /// Apply rejected decisions to opposite-format files on a swapped card.
    CounterpartApply {
        /// Existing run directory containing reviewed decisions.
        #[arg(long)]
        run: PathBuf,
        /// Root folder of the currently mounted counterpart card.
        #[arg(long)]
        card: PathBuf,
        /// Local parent folder for recoverable moved files. Defaults inside the run.
        #[arg(long)]
        destination: Option<PathBuf>,
        /// Confirm the verified copy-then-remove operation.
        #[arg(long)]
        confirm: bool,
    },
    /// Restore moved opposite-format files to a currently mounted card.
    CounterpartRestore {
        /// Existing run directory containing move_state.json.
        #[arg(long)]
        run: PathBuf,
        /// Root folder of the currently mounted counterpart card.
        #[arg(long)]
        card: PathBuf,
        /// Confirm the verified restore operation.
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(Debug, Clone, clap::Args)]
struct ScanArgs {
    /// Long edge used for the first-pass scoring preview.
    #[arg(long, default_value_t = 1280)]
    preview_size: u32,
    /// Long edge used for targeted high-resolution refinement of close candidates.
    #[arg(long, default_value_t = 2048)]
    refine_size: u32,
    /// Maximum high-resolution refinement candidates per burst cluster.
    #[arg(long, default_value_t = 2)]
    refine_candidates_per_cluster: usize,
    /// Disable targeted high-resolution refinement.
    #[arg(long)]
    no_refine: bool,
    /// Long edge used for generated review thumbnails.
    #[arg(long, default_value_t = 320)]
    thumb_size: u32,
    /// Maximum filename counter gap before splitting a burst.
    #[arg(long, default_value_t = 12)]
    max_seq_gap: i64,
    /// Maximum adjacent capture/file-time gap in seconds before splitting a burst.
    #[arg(long, default_value_t = 1.25)]
    max_time_gap: f64,
    /// Maximum total time span in seconds for one burst cluster.
    #[arg(long, default_value_t = 1.80)]
    max_cluster_span: f64,
    /// Maximum whole-frame hash distance used as a fast scene-change guard.
    #[arg(long, default_value_t = 30)]
    max_hash_gap: u32,
    /// Maximum subject-aware visual distance inside one near-duplicate stack.
    #[arg(long, default_value_t = 0.20)]
    max_duplicate_distance: f64,
    /// Minimum confidence required before suggesting an automatic reject.
    #[arg(long, default_value_t = 0.52)]
    min_duplicate_confidence: f64,
    /// Fixed keep count per cluster. Omit for automatic cluster-size-based counts.
    #[arg(long)]
    keepers_per_cluster: Option<usize>,
    /// Legacy compatibility flag. Unique shots remain protected from automatic rejection.
    #[arg(long, hide = true)]
    cull_singletons: bool,
    /// Worker count for parallel scoring. Defaults to available logical CPUs, capped at 8.
    #[arg(long)]
    workers: Option<usize>,
    /// Processing preference. CPU uses the best compatible SIMD implementation.
    #[arg(long, value_enum, default_value_t = AccelArg::Auto)]
    acceleration: AccelArg,
    /// Local subject detector used to improve completeness/out-of-frame scoring.
    #[arg(long, value_enum, default_value_t = DetectorArg::Auto)]
    detector: DetectorArg,
    /// Local ML model size. macOS Vision ignores this option.
    #[arg(long, value_enum, default_value_t = DetectorModelArg::Fast)]
    detector_model: DetectorModelArg,
    /// Execution device for local ML detection. GPU currently means CUDA on Linux.
    #[arg(long, value_enum, default_value_t = DetectorDeviceArg::Auto)]
    detector_device: DetectorDeviceArg,
    /// ONNX Runtime threads used by a serialized local ML detector session.
    #[arg(long)]
    detector_threads: Option<usize>,
    /// Offline model-pack directory created by scripts/install_linux_ml_models.sh.
    #[arg(long, value_name = "DIR", env = "BFD_ML_MODEL_PACK")]
    detector_model_pack: Option<PathBuf>,
    /// Skip thumbnail generation.
    #[arg(long)]
    no_thumbs: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AccelArg {
    Auto,
    Cpu,
    Gpu,
    Portable,
    #[value(hide = true)]
    Avx2,
    #[value(hide = true)]
    Neon,
    #[value(hide = true)]
    Metal,
    #[value(hide = true)]
    Cuda,
    #[value(hide = true)]
    Opencl,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DetectorArg {
    Auto,
    Off,
    Heuristic,
    Ml,
    #[value(hide = true)]
    Vision,
    #[value(hide = true)]
    MlLight,
    #[value(hide = true)]
    MlHeavy,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DetectorModelArg {
    Fast,
    Accurate,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DetectorDeviceArg {
    Auto,
    Cpu,
    Gpu,
    #[value(hide = true)]
    Cuda,
}

impl From<AccelArg> for AccelerationPreference {
    fn from(value: AccelArg) -> Self {
        match value {
            AccelArg::Auto => Self::Auto,
            AccelArg::Cpu => Self::Cpu,
            AccelArg::Gpu => Self::Gpu,
            AccelArg::Portable => Self::Portable,
            AccelArg::Avx2 | AccelArg::Neon => Self::Cpu,
            AccelArg::Metal | AccelArg::Cuda => Self::Gpu,
            AccelArg::Opencl => Self::Portable,
        }
    }
}

impl From<DetectorArg> for DetectorPreference {
    fn from(value: DetectorArg) -> Self {
        match value {
            DetectorArg::Auto => Self::Auto,
            DetectorArg::Off => Self::Off,
            DetectorArg::Heuristic => Self::Heuristic,
            DetectorArg::Ml | DetectorArg::Vision | DetectorArg::MlLight | DetectorArg::MlHeavy => {
                Self::Ml
            }
        }
    }
}

impl From<DetectorModelArg> for DetectorModelPreference {
    fn from(value: DetectorModelArg) -> Self {
        match value {
            DetectorModelArg::Fast => Self::Fast,
            DetectorModelArg::Accurate => Self::Accurate,
        }
    }
}

impl From<DetectorDeviceArg> for DetectorDevicePreference {
    fn from(value: DetectorDeviceArg) -> Self {
        match value {
            DetectorDeviceArg::Auto => Self::Auto,
            DetectorDeviceArg::Cpu => Self::Cpu,
            DetectorDeviceArg::Gpu | DetectorDeviceArg::Cuda => Self::Gpu,
        }
    }
}

impl From<ScanArgs> for ScanOptions {
    fn from(value: ScanArgs) -> Self {
        let detector_model = match value.detector {
            DetectorArg::MlHeavy => DetectorModelPreference::Accurate,
            DetectorArg::MlLight => DetectorModelPreference::Fast,
            _ => value.detector_model.into(),
        };
        Self {
            preview_size: value.preview_size,
            refine_size: value.refine_size,
            refine_candidates_per_cluster: value.refine_candidates_per_cluster,
            disable_refinement: value.no_refine,
            thumb_size: value.thumb_size,
            max_seq_gap: value.max_seq_gap,
            max_time_gap_ms: (value.max_time_gap * 1000.0).round().max(0.0) as i64,
            max_cluster_span_ms: (value.max_cluster_span * 1000.0).round().max(0.0) as i64,
            max_hash_gap: value.max_hash_gap,
            max_duplicate_distance: value.max_duplicate_distance.clamp(0.01, 1.0),
            min_duplicate_confidence: value.min_duplicate_confidence.clamp(0.0, 1.0),
            keepers_per_cluster: value.keepers_per_cluster,
            cull_singletons: value.cull_singletons,
            workers: value.workers,
            acceleration: value.acceleration.into(),
            detector: value.detector.into(),
            detector_model,
            detector_device: value.detector_device.into(),
            detector_threads: value.detector_threads,
            detector_model_pack: value.detector_model_pack,
            generate_thumbnails: !value.no_thumbs,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };
    match command {
        Command::Scan { root, out, options } => {
            let Some(run_dir) = run_cli_scan(root, out, options.into()).await? else {
                return Ok(());
            };
            println!("Wrote run artifacts to {}", run_dir.display());
        }
        Command::App {
            root,
            out,
            host,
            port,
            open,
            options,
        } => {
            let Some(run_dir) = run_cli_scan(root, out, options.into()).await? else {
                return Ok(());
            };
            serve(run_dir, host, port, open).await?;
        }
        Command::Serve {
            run,
            host,
            port,
            open,
        } => {
            serve(run, host, port, open).await?;
        }
        Command::Export { run } => {
            artifacts::export_reviewed_artifacts(&run)
                .with_context(|| format!("exporting reviewed artifacts from {}", run.display()))?;
            println!("Updated CSV exports and move script in {}", run.display());
        }
        Command::Relocate { run, to } => {
            let result = run_storage::relocate_run(&run, &to, |update| {
                let percent = (update.overall_fraction * 100.0).round() as usize;
                let detail = update.detail.as_deref().unwrap_or("Moving run");
                eprint!("\rRelocating [{percent:>3}%] {detail:<60}");
            })
            .with_context(|| format!("relocating {} into {}", run.display(), to.display()))?;
            eprintln!();
            println!("Moved run to {}", result.run_dir.display());
            for warning in result.warnings {
                eprintln!("Warning: {warning}");
            }
        }
        Command::CounterpartPlan { run, card } => {
            let result = plan_counterparts(&run, &card)
                .with_context(|| format!("planning counterpart matches in {}", card.display()))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::CounterpartApply {
            run,
            card,
            destination,
            confirm,
        } => {
            let result = apply_counterparts(&run, &card, destination.as_deref(), confirm)
                .with_context(|| format!("applying decisions to {}", card.display()))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::CounterpartRestore { run, card, confirm } => {
            let result = restore_counterparts(&run, &card, confirm)
                .with_context(|| format!("restoring counterpart files to {}", card.display()))?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

async fn run_cli_scan(
    root: PathBuf,
    out: Option<PathBuf>,
    options: ScanOptions,
) -> anyhow::Result<Option<PathBuf>> {
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let mut task = tokio::task::spawn_blocking(move || {
        run_scan_controlled(
            &root,
            out,
            options,
            terminal_progress_reporter(),
            worker_cancellation,
        )
    });

    tokio::select! {
        result = &mut task => {
            Ok(Some(result.context("joining scan worker")??))
        }
        signal = tokio::signal::ctrl_c() => {
            signal.context("installing Ctrl+C handler")?;
            eprintln!("Cancellation requested; finishing the current photo safely...");
            cancellation.cancel();
            match task.await.context("joining cancelled scan worker")? {
                Ok(run_dir) => Ok(Some(run_dir)),
                Err(error) if is_scan_cancelled(&error) => {
                    eprintln!("Scan cancelled cleanly.");
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }
    }
}

async fn serve(run: PathBuf, host: String, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("invalid bind address {host}:{port}"))?;
    let url = format!("http://{addr}");
    if open_browser {
        let _ = open::that(&url);
    }
    println!("Serving review UI at {url}");
    server::serve(run, addr).await
}
