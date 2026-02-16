use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "contextsmith",
    about = "A deterministic, token-aware context bundler for LLMs",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Project root directory
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    /// Path to config file
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Disable caching
    #[arg(long, global = true)]
    pub no_cache: bool,

    /// Override cache directory
    #[arg(long, global = true)]
    pub cache_dir: Option<PathBuf>,

    /// Number of threads
    #[arg(long, global = true)]
    pub threads: Option<usize>,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Color output mode
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Show timing information
    #[arg(long, global = true)]
    pub time: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize a new contextsmith project
    Init {
        /// Project root directory
        #[arg(long)]
        root: Option<PathBuf>,

        /// Path to write config file
        #[arg(long)]
        config: Option<PathBuf>,

        /// Overwrite existing config
        #[arg(long)]
        force: bool,

        /// Skip cache directory creation
        #[arg(long)]
        no_cache: bool,
    },

    /// Gather context from git diffs
    #[command(alias = "d")]
    Diff {
        /// Git revision range (e.g. HEAD~3..HEAD)
        rev_range: Option<String>,

        /// Include staged changes
        #[arg(long)]
        staged: bool,

        /// Include untracked files
        #[arg(long)]
        untracked: bool,

        /// Changes since timestamp or duration (e.g. "2h", "2024-01-01")
        #[arg(long)]
        since: Option<String>,

        /// Only include hunks, not full files
        #[arg(long)]
        hunks_only: bool,

        /// Lines of context around hunks
        #[arg(long, default_value = "3")]
        context: usize,

        /// Include related symbols (callers, tests)
        #[arg(long)]
        include_related: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,

        /// Write output to file
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Write to stdout
        #[arg(long)]
        stdout: bool,

        /// Token budget
        #[arg(long)]
        budget: Option<usize>,
    },

    /// Collect context by query
    #[command(alias = "c")]
    Collect {
        /// Free-text query or symbol name
        query: Option<String>,

        /// Scope: file, directory, or module
        #[arg(long)]
        scope: Option<String>,

        /// Specific files to include
        #[arg(long)]
        files: Vec<PathBuf>,

        /// Exclude patterns
        #[arg(long)]
        exclude: Vec<String>,

        /// Filter by language
        #[arg(long)]
        lang: Option<String>,

        /// Search for symbol definitions
        #[arg(long)]
        symbol: Option<String>,

        /// Filter by file path pattern
        #[arg(long)]
        path: Option<String>,

        /// Include git diff context
        #[arg(long)]
        diff: Option<String>,

        /// Search by content pattern (grep)
        #[arg(long)]
        grep: Option<String>,

        /// Line span (e.g. "10:50")
        #[arg(long)]
        span: Option<String>,

        /// Max snippets per file
        #[arg(long)]
        max_snippets: Option<usize>,

        /// Max files to include
        #[arg(long)]
        max_files: Option<usize>,

        /// Include definitions of referenced symbols
        #[arg(long)]
        include_defs: bool,

        /// Include references to matched symbols
        #[arg(long)]
        include_refs: bool,

        /// Include import statements
        #[arg(long)]
        include_imports: bool,

        /// Include related test files
        #[arg(long)]
        tests: bool,

        /// Ranking strategy
        #[arg(long)]
        rank: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,

        /// Write output to file
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Write to stdout
        #[arg(long)]
        stdout: bool,

        /// Token budget
        #[arg(long)]
        budget: Option<usize>,
    },

    /// Pack collected context into a token-budgeted bundle
    #[command(alias = "p")]
    Pack {
        /// Input bundle file
        bundle: Option<PathBuf>,

        /// Token budget
        #[arg(long)]
        budget: Option<usize>,

        /// Character budget (alternative to token budget)
        #[arg(long)]
        chars: Option<usize>,

        /// Model name for tokenization
        #[arg(long)]
        model: Option<String>,

        /// Reserve tokens for response
        #[arg(long)]
        reserve: Option<usize>,

        /// Packing strategy
        #[arg(long)]
        strategy: Option<String>,

        /// Must-include files
        #[arg(long)]
        must: Vec<PathBuf>,

        /// Files to drop
        #[arg(long)]
        drop: Vec<PathBuf>,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,

        /// Write to stdout
        #[arg(long)]
        stdout: bool,

        /// Write output to file
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// Trim content to fit a token budget
    Trim {
        /// Input file
        input: Option<PathBuf>,

        /// Token budget
        #[arg(long)]
        budget: Option<usize>,

        /// Character budget
        #[arg(long)]
        chars: Option<usize>,

        /// Model name for tokenization
        #[arg(long)]
        model: Option<String>,

        /// Reserve tokens
        #[arg(long)]
        reserve: Option<usize>,

        /// Keep first N lines
        #[arg(long)]
        keep_head: Option<usize>,

        /// Keep manifest/table-of-contents
        #[arg(long)]
        keep_manifest: bool,

        /// Write output to file
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Write to stdout
        #[arg(long)]
        stdout: bool,
    },

    /// Generate a project map (file tree, symbols, dependency graph)
    Map {
        /// Include full file contents
        #[arg(long)]
        full: bool,

        /// Text-only output
        #[arg(long)]
        text: bool,

        /// Include symbol index
        #[arg(long)]
        symbols: bool,

        /// Include dependency graph
        #[arg(long)]
        graph: bool,

        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,

        /// Write output to file
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Watch for file changes
        #[arg(long)]
        watch: bool,
    },

    /// Show statistics for a context bundle
    Stats {
        /// Input bundle file
        bundle: Option<PathBuf>,

        /// Show top N files by token count
        #[arg(long)]
        top_files: Option<usize>,

        /// Group stats by language
        #[arg(long)]
        by_lang: bool,

        /// Group stats by file type
        #[arg(long)]
        by_type: bool,

        /// Show token counts
        #[arg(long)]
        tokens: bool,
    },

    /// Explain how a context bundle was assembled
    #[command(alias = "e")]
    Explain {
        /// Input bundle file
        bundle: Option<PathBuf>,

        /// Show detailed explanations
        #[arg(long)]
        verbose: bool,

        /// Show top N items
        #[arg(long)]
        top: Option<usize>,

        /// Show ranking weights used
        #[arg(long)]
        show_weights: bool,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Markdown,
    Json,
    Xml,
    Plain,
}
