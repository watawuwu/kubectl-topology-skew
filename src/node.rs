use std::collections::BTreeSet;

use anyhow::*;

use kube::Client;

use crate::{
    arg::NodeOptions, only_node_running, spreading_status, CachedNodeApi, TopologyTable,
    TopologyTables,
};

pub async fn node(opts: NodeOptions, cli: Client) -> Result<TopologyTables> {
    let node_api = CachedNodeApi::try_from(cli.clone()).await?;
    let labels = opts.labels();
    let nodes = node_api.list(&labels).await;
    let nodes = only_node_running(nodes);

    if nodes.is_empty() {
        bail!("No found nodes");
    }

    let (topology_values, domains) =
        spreading_status(&nodes, &opts.topology_key, &node_api).await?;
    let table = TopologyTable::create(topology_values, &domains, None);

    Ok(TopologyTables::from(BTreeSet::from([table])))
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Node;
    use kube::{
        api::{ListMeta, ObjectList, TypeMeta},
        Client,
    };
    use serde::Deserialize;

    use crate::{kube::tests::create_objects, Label};

    use super::*;
    use futures::pin_mut;
    use http::{Request, Response};
    use kube::client::Body;
    use tower_test::mock;

    use crate::arg::NodeOptions;

    #[tokio::test]
    async fn node_ok() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/node_ok_nodes.yaml", Node);
            Ok(())
        });
        let cli = Client::new(mock_service, "default");
        let opts = NodeOptions {
            selector: Vec::new(),
            ..Default::default()
        };

        let topology_tables = node(opts, cli).await?;
        spawned.await??;

        for topology_table in topology_tables {
            assert!(topology_table.header.is_none());
            let mut iter = topology_table.topologies.into_iter();

            let topology1 = iter.next().unwrap();
            assert_eq!(topology1.key, "asia-northeast1-a");
            assert_eq!(topology1.count, 2);
            assert_eq!(topology1.skew, 1);

            let topology2 = iter.next().unwrap();
            assert_eq!(topology2.key, "asia-northeast1-b");
            assert_eq!(topology2.count, 1);
            assert_eq!(topology2.skew, 0);

            let topology3 = iter.next().unwrap();
            assert_eq!(topology3.key, "asia-northeast1-c");
            assert_eq!(topology3.count, 1);
            assert_eq!(topology3.skew, 0);
        }

        Ok(())
    }

    #[tokio::test]
    async fn node_selector() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/node_selector_nodes.yaml", Node);
            Ok(())
        });
        let cli = Client::new(mock_service, "default");
        let opts = NodeOptions {
            selector: vec![Label::from(("kubernetes.io/os", "linux"))],
            ..Default::default()
        };

        let topology_tables = node(opts, cli).await?;
        spawned.await??;

        for topology_table in topology_tables {
            assert!(topology_table.header.is_none());
            let mut iter = topology_table.topologies.into_iter();

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
        }

        Ok(())
    }

    #[tokio::test]
    async fn node_notfound() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/empty.yaml", Node);
            Ok(())
        });
        let cli = Client::new(mock_service, "default");
        let opts = NodeOptions {
            selector: Vec::new(),
            ..Default::default()
        };

        let result = node(opts, cli).await;
        spawned.await??;

        // TODO
        assert!(result.is_err());

        Ok(())
    }

    // #[tokio::test]
    // async fn pod_ok() -> Result<()> {
    //     let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
    //     let spawned = tokio::spawn(async move {
    //         pin_mut!(handle);
    //         create_objects!(handle, "../tests/case3_node.yaml", Node);
    //         create_objects!(handle, "../tests/case3_pod.yaml", Pod);
    //         Ok(())
    //     });

    //     let ns = "default";
    //     let cli = Client::new(mock_service, ns);
    //     let opts = PodOptions {
    //         namespace: Some(ns.to_owned()),
    //         selector: None,
    //     };

    //     let topologies = pod(cli, opts).await?;

    //     assert!(topologies.exists());
    //     assert!(topologies.has_name());

    //     let mut topologies_iter = topologies.into_iter();

    //     // apps/v1/daemonset/node-exporter
    //     // ====================================================================
    //     let topology1 = topologies_iter.next().unwrap();
    //     assert_eq!(topology1.name(), "apps/v1/daemonset/node-exporter");

    //     let mut iter = topology1.regions.into_iter();
    //     let region1 = iter.next().unwrap();
    //     assert_eq!(region1.name, "asia-northeast1");

    //     let mut iter = region1.zones.into_iter();
    //     let zone1 = iter.next().unwrap();
    //     assert_eq!(zone1.name, "asia-northeast1-a");
    //     assert_eq!(zone1.count, 1);
    //     assert_eq!(zone1.skew, 0);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-b");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 0);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-c");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 0);

    //     // apps/v1/replicaset/nginx-56fcdc489c
    //     // ====================================================================
    //     let topology2 = topologies_iter.next().unwrap();
    //     assert_eq!(topology2.name(), "apps/v1/replicaset/nginx-56fcdc489c");

    //     let mut iter = topology2.regions.into_iter();
    //     let region1 = iter.next().unwrap();
    //     assert_eq!(region1.name, "asia-northeast1");

    //     let mut iter = region1.zones.into_iter();
    //     let zone1 = iter.next().unwrap();
    //     assert_eq!(zone1.name, "asia-northeast1-a");
    //     assert_eq!(zone1.count, 3);
    //     assert_eq!(zone1.skew, 2);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-b");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 0);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-c");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 0);

    //     // apps/v1/statefulset/mysql
    //     // ====================================================================
    //     let topology3 = topologies_iter.next().unwrap();
    //     assert_eq!(topology3.name(), "apps/v1/statefulset/mysql");

    //     let mut iter = topology3.regions.into_iter();
    //     let region1 = iter.next().unwrap();
    //     assert_eq!(region1.name, "asia-northeast1");

    //     let mut iter = region1.zones.into_iter();
    //     let zone1 = iter.next().unwrap();
    //     assert_eq!(zone1.name, "asia-northeast1-a");
    //     assert_eq!(zone1.count, 1);
    //     assert_eq!(zone1.skew, 1);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-b");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 1);

    //     // batch/v1/job/batch
    //     // ====================================================================
    //     let topology3 = topologies_iter.next().unwrap();
    //     assert_eq!(topology3.name(), "batch/v1/job/batch");

    //     let mut iter = topology3.regions.into_iter();
    //     let region1 = iter.next().unwrap();
    //     assert_eq!(region1.name, "asia-northeast1");

    //     let mut iter = region1.zones.into_iter();
    //     let zone1 = iter.next().unwrap();
    //     assert_eq!(zone1.name, "asia-northeast1-a");
    //     assert_eq!(zone1.count, 2);
    //     assert_eq!(zone1.skew, 2);

    //     let zone2 = iter.next().unwrap();
    //     assert_eq!(zone2.name, "asia-northeast1-b");
    //     assert_eq!(zone2.count, 1);
    //     assert_eq!(zone2.skew, 1);

    //     spawned.await??;

    //     Ok(())
    // }

    // #[tokio::test]
    // async fn pod_notfound() -> Result<()> {
    //     let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
    //     let spawned = tokio::spawn(async move {
    //         pin_mut!(handle);
    //         create_objects!(handle, "../tests/case4_node.yaml", Node);
    //         create_objects!(handle, "../tests/case4_pod_empty.yaml", Pod);
    //         Ok(())
    //     });

    //     let namespace = String::from("default");
    //     let cli = Client::new(mock_service, &namespace);
    //     let opts = PodOptions {
    //         namespace: Some(namespace),
    //         selector: None,
    //     };

    //     let topologies = pod(cli, opts).await?;
    //     spawned.await??;
    //     assert!(!topologies.exists());

    //     Ok(())
    // }
    //}
}
