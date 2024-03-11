use crate::kube::{Label, LabelSelector};
use anyhow::*;
use clap::builder::{
    styling::{AnsiColor, Effects},
    Styles,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::fmt::{Display, Formatter};
use strum::AsRefStr;

fn help_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
}

fn parse_key_val(s: &str) -> Result<Label> {
    let pos = s
        .find('=')
        .context("Not found `=` in key value pair(KEY=VALUE)")?;
    Ok(Label(s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[derive(Parser, Debug)]
#[command(author, version, about, styles(help_styles()))]
pub struct Args {
    #[command(flatten)]
    pub kube_options: KubeConfigOptions,

    /// Output format
    #[arg(short, long, default_value_t = OutputFormat::Text, global = true)]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub(crate) sub: SubCommand,
}

#[derive(Subcommand, Debug)]
pub enum SubCommand {
    /// Print pod topology skew
    Pod {
        #[command(flatten)]
        pod_options: PodOptions,
    },
    /// Print node topology skew
    Node {
        #[command(flatten)]
        node_options: NodeOptions,
    },
}

#[derive(Debug, Parser)]
pub struct PodOptions {
    /// Kubernetes namespace name
    #[arg(short, long, global = true)]
    pub namespace: Option<String>,

    /// Label selector for pod list
    #[arg(short = 'l', long, value_parser = parse_key_val)]
    pub selector: Option<Vec<Label>>,
}

impl PodOptions {
    pub fn selectors(&self) -> String {
        self.selector
            .as_ref()
            .map(LabelSelector::selector)
            .unwrap_or_default()
    }

    pub fn is_selector(&self) -> bool {
        self.selector.is_some()
    }

    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[derive(Debug, Parser)]
pub struct NodeOptions {
    /// Label selector for pod list
    #[arg(short = 'l', long, value_parser = parse_key_val)]
    pub selector: Option<Vec<Label>>,
}

impl NodeOptions {
    pub fn selectors(&self) -> String {
        self.selector
            .as_ref()
            .map(LabelSelector::selector)
            .unwrap_or_default()
    }
}

#[derive(Debug, Parser)]
pub struct KubeConfigOptions {
    /// Kubernetes config context
    #[arg(long, global = true)]
    pub context: Option<String>,

    /// Kubernetes config cluster
    #[arg(long, global = true)]
    pub cluster: Option<String>,

    /// Kubernetes config user
    #[arg(long, global = true)]
    pub user: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Yaml,
    Json,
    Tree,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Args::command().debug_assert()
}
