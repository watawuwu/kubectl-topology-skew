use std::collections::BTreeMap;

use crate::{arg::ResourceOptions, topology_table_find_by, TopologyTables};
use anyhow::*;
use kube::Client;

pub async fn pod(opts: ResourceOptions, cli: Client) -> Result<TopologyTables> {
    let namespace = opts.namespace().unwrap_or(cli.default_namespace());
    let selectors = opts.selectors();
    let topology_key = &opts.topology_key;
    let labels_map = BTreeMap::from([(String::new(), selectors)]);
    let use_header = false;

    let tables =
        topology_table_find_by(labels_map, namespace, topology_key, cli.clone(), use_header)
            .await?;

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::{Node, Pod};
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

    #[tokio::test]
    async fn pod_no_options() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/pod_no_options_pods.yaml", Pod);
            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = ResourceOptions {
            namespace: Some(ns.to_owned()),
            ..Default::default()
        };

        let topology_tables = pod(opts, cli).await?;

        let mut topology_table_iter = topology_tables.into_iter();

        let topology_table1 = topology_table_iter.next().unwrap();
        assert_eq!(topology_table1.header, None);

        let mut iter = topology_table1.topologies.into_iter();
        let topology1 = iter.next().unwrap();

        assert_eq!(topology1.key, "asia-northeast1-a");
        assert_eq!(topology1.count, 7);
        assert_eq!(topology1.skew, 5);

        let topology2 = iter.next().unwrap();
        assert_eq!(topology2.key, "asia-northeast1-b");
        assert_eq!(topology2.count, 4);
        assert_eq!(topology2.skew, 2);

        let topology3 = iter.next().unwrap();
        assert_eq!(topology3.key, "asia-northeast1-c");
        assert_eq!(topology3.count, 2);
        assert_eq!(topology3.skew, 0);

        spawned.await??;

        Ok(())
    }

    #[tokio::test]
    async fn pod_one_domain() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/pod_one_domain_pods.yaml", Pod);
            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = ResourceOptions {
            namespace: Some(ns.to_owned()),
            ..Default::default()
        };

        let topology_tables = pod(opts, cli).await?;

        let mut topology_table_iter = topology_tables.into_iter();

        let topology_table1 = topology_table_iter.next().unwrap();
        assert_eq!(topology_table1.header, None);

        let mut iter = topology_table1.topologies.into_iter();
        let topology1 = iter.next().unwrap();

        assert_eq!(topology1.key, "asia-northeast1-a");
        assert_eq!(topology1.count, 3);
        assert_eq!(topology1.skew, 3);

        let topology2 = iter.next().unwrap();
        assert_eq!(topology2.key, "asia-northeast1-b");
        assert_eq!(topology2.count, 0);
        assert_eq!(topology2.skew, 0);

        let topology3 = iter.next().unwrap();
        assert_eq!(topology3.key, "asia-northeast1-c");
        assert_eq!(topology3.count, 0);
        assert_eq!(topology3.skew, 0);

        spawned.await??;

        Ok(())
    }

    #[tokio::test]
    async fn pod_selector() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/pod_selector_pods.yaml", Pod);
            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = ResourceOptions {
            namespace: Some(ns.to_owned()),
            selector: vec![Label::from(("app", "app-a"))],
            ..Default::default()
        };

        let topology_tables = pod(opts, cli).await?;

        let mut topology_table_iter = topology_tables.into_iter();

        let topology_table1 = topology_table_iter.next().unwrap();
        assert_eq!(topology_table1.header, None);

        let mut iter = topology_table1.topologies.into_iter();
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

        spawned.await??;

        Ok(())
    }

    #[tokio::test]
    async fn pod_selectors() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/pod_selectors_pods.yaml", Pod);
            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = ResourceOptions {
            namespace: Some(ns.to_owned()),
            selector: vec![
                Label::from(("app", "app-a")),
                Label::from(("group", "group-a")),
            ],
            ..Default::default()
        };

        let topology_tables = pod(opts, cli).await?;

        let mut topology_table_iter = topology_tables.into_iter();

        let topology_table1 = topology_table_iter.next().unwrap();
        assert_eq!(topology_table1.header, None);

        let mut iter = topology_table1.topologies.into_iter();
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

        spawned.await??;

        Ok(())
    }

    #[tokio::test]
    async fn pod_notfound() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/nodes.yaml", Node);
            create_objects!(handle, "../tests/empty.yaml", Pod);
            Ok(())
        });

        let namespace = String::from("default");
        let cli = Client::new(mock_service, &namespace);
        let opts = ResourceOptions {
            namespace: Some(namespace),
            ..Default::default()
        };

        let result = pod(opts, cli).await;
        spawned.await??;

        // TODO
        assert!(result.is_err());

        Ok(())
    }
}
