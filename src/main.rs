mod all;
mod arg;
mod daemonset;
mod deployment;
mod job;
mod kube;
mod node;
mod pod;
mod statefulset;
mod topology;
mod view;

use crate::all::all;
use crate::arg::{Args, SubCommand};
use crate::daemonset::daemonset;
use crate::deployment::deployment;
use crate::job::job;
use crate::kube::*;
use crate::node::node;
use crate::pod::pod;
use crate::statefulset::statefulset;
use crate::topology::*;
use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Generator};

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    let kopts = args.kube_options;
    let cli = kube_client(kopts.context, kopts.cluster, kopts.user).await?;

    let topologies = match args.sub {
        SubCommand::Pod { options } => pod(options, cli.clone()).await?,
        SubCommand::Node { options } => node(options, cli.clone()).await?,
        SubCommand::Deployment { options } => deployment(options, cli.clone()).await?,
        SubCommand::StatefulSet { options } => statefulset(options, cli.clone()).await?,
        SubCommand::DaemonSet { options } => daemonset(options, cli.clone()).await?,
        SubCommand::Job { options } => job(options, cli.clone()).await?,
        SubCommand::All { options } => all(options, cli.clone()).await?,
        SubCommand::Completion { shell } => {
            print_completions(shell);
            return Ok(());
        }
    };
    let text = view::out(topologies, args.output)?;

    println!("{text}");

    Ok(())
}

fn print_completions<G: Generator>(gen: G) {
    let mut cmd = Args::command();
    let name = cmd.get_name().to_owned();

    generate(gen, &mut cmd, name, &mut std::io::stdout());
}
