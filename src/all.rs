use std::collections::BTreeMap;

use crate::{
    arg::ResourceOptions, daemonset, deployment, job, nodes_by, pods_by, resources,
    spreading_status, statefulset, CachedNodeApi, TopologyTable, TopologyTables,
};
use anyhow::*;
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    batch::v1::Job,
};
use kube::Client;

pub async fn all(opts: ResourceOptions, cli: Client) -> Result<TopologyTables> {
    let namespace = opts.namespace().unwrap_or(cli.default_namespace());
    let selectors = opts.selectors();
    let topology_key = &opts.topology_key;

    let mut labels_set: BTreeMap<String, String> = BTreeMap::new();

    let deployments =
        resources::<Deployment>(None, namespace, Some(&selectors), cli.clone()).await?;
    labels_set.extend(deployment::labels_set_by(&deployments)?);

    let statefulsets =
        resources::<StatefulSet>(None, namespace, Some(&selectors), cli.clone()).await?;
    labels_set.extend(statefulset::labels_set_by(&statefulsets)?);

    let jobs = resources::<Job>(None, namespace, Some(&selectors), cli.clone()).await?;
    labels_set.extend(job::labels_set_by(&jobs)?);

    let daemonsets = resources::<DaemonSet>(None, namespace, Some(&selectors), cli.clone()).await?;
    labels_set.extend(daemonset::labels_set_by(&daemonsets)?);

    let mut tables = TopologyTables::default();

    let node_api = CachedNodeApi::try_from(cli.clone()).await?;

    for (name, labels) in labels_set {
        let pods = pods_by(&[&labels], namespace, cli.clone()).await?;
        let nodes = nodes_by(&pods, &node_api).await?;
        let (topology_values, domains) = spreading_status(&nodes, topology_key, &node_api).await?;
        let table = TopologyTable::create(topology_values, &domains, Some(name));

        tables.insert(table);
    }

    Ok(tables)
}
