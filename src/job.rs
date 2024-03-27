use std::collections::BTreeMap;

use anyhow::*;
use itertools::*;
use k8s_openapi::api::batch::v1::Job;
use kube::{api::TypeMeta, Client, ResourceExt};

use crate::{arg::ResourceWithNameOptions, resources, topology_table_find_by, TopologyTables};

pub async fn job(opts: ResourceWithNameOptions, cli: Client) -> Result<TopologyTables> {
    let name = opts.name();
    let namespace = opts.namespace().unwrap_or(cli.default_namespace());
    let selectors = opts.selectors();
    let jobs = resources::<Job>(name, namespace, selectors.as_deref(), cli.clone()).await?;

    if jobs.is_empty() {
        bail!("No found job");
    }

    let labels_map = labels_set_by(&jobs)?;
    let topology_key = &opts.topology_key;
    let tables = topology_table_find_by(
        labels_map,
        namespace,
        topology_key,
        cli.clone(),
        name.is_none(),
    )
    .await?;

    Ok(tables)
}

pub fn labels_set_by(jobs: &[Job]) -> Result<BTreeMap<String, String>> {
    let job_to_lables = |job: &Job| {
        let selector = &job
            .spec
            .as_ref()
            .and_then(|spec| spec.selector.as_ref())
            .context("No found label selector")?;

        let labels = selector
            .match_labels
            .as_ref()
            .map(|x| x.iter().map(|(k, v)| format!("{}={}", k, v)).join(","))
            .context("No found selector")?;

        let meta = TypeMeta::resource::<Job>();
        let api_version = meta.api_version;
        let kind = meta.kind.to_lowercase();
        let name = format!("{}/{}/{}", api_version, kind, job.name_any());

        Ok((name, labels))
    };

    let labels = jobs
        .iter()
        .map(job_to_lables)
        .collect::<Result<BTreeMap<_, _>>>()?;

    Ok(labels)
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::{Node, Pod};
    use kube::{
        api::{ListMeta, ObjectList, TypeMeta},
        Client,
    };
    use serde::Deserialize;

    use crate::kube::tests::create_objects;

    use super::*;
    use futures::pin_mut;
    use http::{Request, Response};
    use kube::client::Body;
    use tower_test::mock;

    #[tokio::test]
    async fn job_no_options() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/job_no_options_job.yaml", Job);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/job_no_options_pods1.yaml", Pod);
            create_objects!(handle, "../tests/job_no_options_pods2.yaml", Pod);

            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = ResourceWithNameOptions {
            namespace: Some(ns.to_owned()),
            ..Default::default()
        };

        let topology_tables = job(opts, cli).await?;

        let mut topology_table_iter = topology_tables.into_iter();

        let topology_table1 = topology_table_iter.next().unwrap();
        assert_eq!(
            topology_table1.header,
            Some(String::from("batch/v1/job/job1"))
        );

        let mut iter = topology_table1.topologies.into_iter();
        let topology1 = iter.next().unwrap();

        assert_eq!(topology1.key, "asia-northeast1-a");
        assert_eq!(topology1.count, 1);
        assert_eq!(topology1.skew, 0);

        let topology2 = iter.next().unwrap();
        assert_eq!(topology2.key, "asia-northeast1-b");
        assert_eq!(topology2.count, 1);
        assert_eq!(topology2.skew, 0);

        let topology3 = iter.next().unwrap();
        assert_eq!(topology3.key, "asia-northeast1-c");
        assert_eq!(topology3.count, 1);
        assert_eq!(topology3.skew, 0);

        let topology_table2 = topology_table_iter.next().unwrap();
        assert_eq!(
            topology_table2.header,
            Some(String::from("batch/v1/job/job2"))
        );

        let mut iter = topology_table2.topologies.into_iter();
        let topology1 = iter.next().unwrap();

        assert_eq!(topology1.key, "asia-northeast1-a");
        assert_eq!(topology1.count, 1);
        assert_eq!(topology1.skew, 0);

        let topology2 = iter.next().unwrap();
        assert_eq!(topology2.key, "asia-northeast1-b");
        assert_eq!(topology2.count, 1);
        assert_eq!(topology2.skew, 0);

        let topology3 = iter.next().unwrap();
        assert_eq!(topology3.key, "asia-northeast1-c");
        assert_eq!(topology3.count, 1);
        assert_eq!(topology3.skew, 0);

        spawned.await??;

        Ok(())
    }
}
