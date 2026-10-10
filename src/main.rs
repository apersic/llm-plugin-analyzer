use clap::Parser;
use eframe::egui;
use llm_plugin_analyzer::catalog::{self, Catalog};
use llm_plugin_analyzer::domain::{Query, Scope};
use llm_plugin_analyzer::graph;
use llm_plugin_analyzer::ledger::Scanner;
use llm_plugin_analyzer::project;
use llm_plugin_analyzer::report;
use llm_plugin_analyzer::ui::App;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Live node grid of how an LLM plugin fires on each prompt")]
struct Args {
    #[arg(long)]
    plugin_root: Option<PathBuf>,
    #[arg(long)]
    plugin: Option<String>,
    #[arg(long)]
    cursor_home: Option<PathBuf>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    chat: Option<String>,
    #[arg(long)]
    report: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    shot: Option<PathBuf>,
    #[arg(long, default_value_t = 1280)]
    width: u32,
    #[arg(long, default_value_t = 800)]
    height: u32,
    #[arg(long)]
    timings: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cursor_home = args.cursor_home.clone().unwrap_or_else(default_cursor_home);
    let plugins = catalog::discover(&cursor_home);
    let plugin_root = pick_plugin_root(args.plugin_root.clone(), args.plugin.as_deref(), &plugins);
    let catalog = Catalog::load(plugin_root)?;
    let query = Query {
        scope: match (args.workspace, args.chat) {
            (None, None) => Scope::All,
            (Some(workspace), None) => Scope::Workspace(workspace),
            (workspace, Some(conversation)) => Scope::Chat {
                workspace: workspace.unwrap_or_default(),
                conversation,
            },
        },
    };

    if args.report || args.json || args.shot.is_some() || args.timings {
        let mut scanner = Scanner::default();
        let t0 = std::time::Instant::now();
        let snap = scanner.refresh(&cursor_home, &catalog)?;
        let scan_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = std::time::Instant::now();
        let board = project::board(&snap, &catalog, &query);
        let board_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let t2 = std::time::Instant::now();
        let layout = graph::layout(&board);
        let layout_ms = t2.elapsed().as_secs_f64() * 1000.0;
        let t_stamp = std::time::Instant::now();
        let _ = Scanner::disk_stamp(&cursor_home);
        let stamp_ms = t_stamp.elapsed().as_secs_f64() * 1000.0;
        let t3 = std::time::Instant::now();
        let snap2 = scanner.refresh(&cursor_home, &catalog)?;
        let scan2_ms = t3.elapsed().as_secs_f64() * 1000.0;
        let t4 = std::time::Instant::now();
        let _ = project::board(&snap2, &catalog, &query);
        let board2_ms = t4.elapsed().as_secs_f64() * 1000.0;
        if args.timings {
            eprintln!(
                "timings scan={scan_ms:.1}ms scan2={scan2_ms:.1}ms stamp={stamp_ms:.1}ms board={board_ms:.1}ms board2={board2_ms:.1}ms layout={layout_ms:.1}ms events={} nodes={}",
                snap.events.len(),
                layout.nodes.len()
            );
        }
        if args.json {
            let report = report::from_board(&board);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        if args.report {
            report::print_text(&board);
        }
        if let Some(path) = args.shot {
            graph::write_png(&layout, args.width, args.height, &path)?;
            println!("wrote {} ({} nodes)", path.display(), layout.nodes.len());
        }
        return Ok(());
    }

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    let app = App::new(catalog, plugins, cursor_home, query);
    eframe::run_native(
        "llm-plugin-analyzer",
        native,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn pick_plugin_root(
    plugin_root: Option<PathBuf>,
    plugin: Option<&str>,
    plugins: &[catalog::PluginRef],
) -> PathBuf {
    if let Some(root) = plugin_root {
        return root;
    }
    if let Some(name) = plugin {
        if let Some(found) = plugins.iter().find(|p| p.id == name || p.name == name) {
            return found.root.clone();
        }
    }
    plugins
        .iter()
        .find(|p| p.id == "guide-to-the-code")
        .or_else(|| plugins.first())
        .map(|p| p.root.clone())
        .unwrap_or_else(default_plugin_root)
}

fn default_plugin_root() -> PathBuf {
    dirs_fallback()
        .join(".cursor")
        .join("plugins")
        .join("local")
        .join("guide-to-the-code")
}

fn default_cursor_home() -> PathBuf {
    dirs_fallback().join(".cursor")
}

fn dirs_fallback() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
