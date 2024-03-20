use crate::kube::{Label, LabelSelector};
use anyhow::*;
use clap::builder::{
    styling::{AnsiColor, Effects},
    Styles,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
};
use strum::AsRefStr;

const DEFAULT_ZONE_LABEL: &str = "topology.kubernetes.io/zone";

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
    #[arg(short, long, global = true, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub(crate) sub: SubCommand,
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

#[derive(Subcommand, Debug)]
pub enum SubCommand {
    /// Print pod topology skew
    #[command(visible_alias("po"))]
    Pod {
        #[command(flatten)]
        options: ResourceOptions,
    },
    /// Print deployment topology skew
    #[command(visible_alias("deploy"))]
    Deployment {
        #[command(flatten)]
        options: ResourceWithNameOptions,
    },
    /// Print statefulset topology skew
    #[command(name = "statefulset", visible_alias("sts"))]
    StatefulSet {
        #[command(flatten)]
        options: ResourceWithNameOptions,
    },
    /// Print daemonset topology skew
    #[command(name = "daemonset", visible_alias("ds"))]
    DaemonSet {
        #[command(flatten)]
        options: ResourceWithNameOptions,
    },
    /// Print daemonset topology skew
    Job {
        #[command(flatten)]
        options: ResourceWithNameOptions,
    },
    /// Print node topology skew
    All {
        #[command(flatten)]
        options: ResourceOptions,
    },
    /// Print node topology skew
    #[command(visible_alias("no"))]
    Node {
        #[command(flatten)]
        options: NodeOptions,
    },
}

#[derive(Debug, Parser)]
pub struct ResourceOptions {
    /// Kubernetes namespace name
    #[arg(short, long, global = true)]
    pub namespace: Option<String>,

    /// Topology key
    #[arg(short, long, default_value = DEFAULT_ZONE_LABEL)]
    pub topology_key: String,

    /// Label selector for pod list
    #[arg(short = 'l', long, value_parser = parse_key_val)]
    pub selector: Vec<Label>,
}

impl Default for ResourceOptions {
    fn default() -> Self {
        Self {
            namespace: None,
            topology_key: DEFAULT_ZONE_LABEL.to_string(),
            selector: Vec::new(),
        }
    }
}

impl ResourceOptions {
    pub fn selectors(&self) -> String {
        self.selector.selector()
    }

    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
}

#[derive(Debug, Parser)]
pub struct ResourceWithNameOptions {
    /// Kubernetes namespace name
    #[arg(short, long, global = true)]
    pub namespace: Option<String>,

    /// Topology key
    #[arg(short, long, default_value = DEFAULT_ZONE_LABEL)]
    pub topology_key: String,

    /// Label selector for pod list
    #[arg(short = 'l', long, value_parser = parse_key_val)]
    pub selector: Vec<Label>,

    /// Object name
    pub name: Option<String>,
}

impl Default for ResourceWithNameOptions {
    fn default() -> Self {
        Self {
            namespace: None,
            topology_key: DEFAULT_ZONE_LABEL.to_string(),
            selector: Vec::new(),
            name: None,
        }
    }
}

impl ResourceWithNameOptions {
    pub fn selectors(&self) -> Option<String> {
        let s = self.selector.selector();
        // empty string to None
        (!s.is_empty()).then_some(s)
    }

    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Debug, Parser)]
pub struct NodeOptions {
    /// Topology key
    #[arg(short, long, default_value = DEFAULT_ZONE_LABEL)]
    pub topology_key: String,

    /// Label selector for pod list
    #[arg(short = 'l', long, value_parser = parse_key_val)]
    pub selector: Vec<Label>,
}

impl NodeOptions {
    pub fn labels(&self) -> BTreeMap<String, String> {
        self.selector.labels()
    }
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            topology_key: DEFAULT_ZONE_LABEL.to_string(),
            selector: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Yaml,
    Json,
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
