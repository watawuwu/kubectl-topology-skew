mod arg;
mod kube;
mod topology;
mod view;

use crate::arg::{Args, SubCommand};
use crate::kube::*;
use crate::topology::*;
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    let kopts = args.kube_options;
    let cli = kube_client(kopts.context, kopts.cluster, kopts.user).await?;

    let topologies = match args.sub {
        SubCommand::Pod { pod_options } => pod(cli.clone(), pod_options).await?,
        SubCommand::Node { node_options } => node(cli.clone(), node_options).await?,
    };
    let text = view::out(topologies, args.output)?;

    println!("{text}");

    Ok(())
}
