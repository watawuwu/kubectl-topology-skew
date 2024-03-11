use crate::{
    arg::{NodeOptions, PodOptions},
    kube::*,
};
use ::kube::{
    api::{Api, ListParams},
    Client,
};
use anyhow::*;
use futures::future;
use itertools::Itertools;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::ResourceExt;
use serde::Serialize;
use std::{
    collections::{btree_set, BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::{Display, Formatter},
};

#[derive(Debug, Default, Serialize, PartialEq, PartialOrd)]
pub struct Topologies(BTreeSet<Topology>);

impl Topologies {
    pub fn exists(&self) -> bool {
        !self.0.is_empty() && self.0.iter().all(|topology| topology.exists())
    }

    pub fn has_name(&self) -> bool {
        !self.0.is_empty() && self.0.iter().any(|topology| topology.name.is_some())
    }
}

impl FromIterator<Topology> for Topologies {
    fn from_iter<I: IntoIterator<Item = Topology>>(iter: I) -> Self {
        Topologies(iter.into_iter().collect::<BTreeSet<Topology>>())
    }
}

impl IntoIterator for Topologies {
    type Item = Topology;
    type IntoIter = btree_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Default, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Topology {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub regions: BTreeSet<Region>,
}

impl Topology {
    fn new(regions: BTreeSet<Region>) -> Topology {
        Topology {
            name: None,
            regions,
        }
    }

    fn with_name(name: Option<String>, regions: BTreeSet<Region>) -> Topology {
        Topology { name, regions }
    }

    pub fn name(&self) -> String {
        self.name.clone().unwrap_or_default()
    }

    fn exists(&self) -> bool {
        !self.regions.is_empty()
    }
}

fn nodes2region(iter: Vec<Node>, all_zones: HashSet<String>) -> BTreeSet<Region> {
    iter.into_iter()
        .map(|node| (region(&node), zone(&node)))
        .into_group_map()
        .into_iter()
        .map(|(region, zones)| Region::new(region, zones, &all_zones))
        .collect::<BTreeSet<_>>()
}

#[derive(Debug, Default, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Region {
    pub name: String,
    pub zones: BTreeSet<Zone>,
}

impl Region {
    fn new(name: String, scheduled_zone: Vec<String>, all_zones: &HashSet<String>) -> Self {
        let mut count_map = all_zones
            .iter()
            .map(|name| (name.clone(), 0usize))
            .collect::<HashMap<_, _>>();

        let scheduled_count_map = scheduled_zone.into_iter().counts();

        // merge count map
        count_map.extend(scheduled_count_map);

        let min_count_in_region = count_map.values().min().copied().unwrap_or(0);
        let zones = count_map
            .into_iter()
            .map(|(zone_name, count)| {
                Zone::new(zone_name, count as u32, min_count_in_region as u32)
            })
            .collect::<BTreeSet<_>>();
        Region { name, zones }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Zone {
    pub name: String,
    pub count: u32,
    pub skew: u32,
}

impl Zone {
    fn new(name: String, count: u32, min_count_in_region: u32) -> Self {
        let skew = count - min_count_in_region;
        Self { name, count, skew }
    }
}

impl Display for Zone {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let info = format!(
            "{} => count: {} skew: {}",
            &self.name, &self.count, &self.skew
        );

        write!(f, "{}", info)
    }
}

pub async fn pod(cli: Client, opts: PodOptions) -> Result<Topologies> {
    let node_api = CachedNodeApi::try_from(cli.clone()).await?;
    let label_selector = opts.selectors();

    let params = ListParams::default().labels(&label_selector);
    let ns = opts.namespace().unwrap_or(cli.default_namespace());
    let pod_api: Api<Pod> = Api::namespaced(cli.clone(), ns);

    let pods = only_running(pod_api.list(&params).await?);

    let grouped_pods: BTreeMap<Option<String>, Vec<Pod>> = if opts.is_selector() {
        BTreeMap::from([(None, pods)])
    } else {
        group_by_owner_abbr(pods)
    };

    let topology_fut = grouped_pods
        .into_iter()
        .map(|(name, pods)| async { topology(name, pods, &node_api).await })
        .collect::<Vec<_>>();

    let topologies = future::join_all(topology_fut)
        .await
        .into_iter()
        .collect::<Result<Topologies>>()?;

    Ok(topologies)
}

async fn topology(
    name: Option<String>,
    pods: Vec<Pod>,
    node_api: &CachedNodeApi,
) -> Result<Topology> {
    let nodes_fut = pods
        .iter()
        .filter_map(|pod| pod.spec.as_ref().and_then(|spec| spec.node_name.clone()))
        .map(|node_name| async move { node_api.get(&node_name).await })
        .collect::<Vec<_>>();

    let nodes = future::join_all(nodes_fut)
        .await
        .into_iter()
        .flatten()
        .filter(|node| node.labels().get(ZONE_LABEL).is_some())
        .collect::<Vec<_>>();

    let all_zones = node_api.all_zones();
    let regions = nodes2region(nodes, all_zones);
    let topology = Topology::with_name(name, regions);

    Ok(topology)
}

pub async fn node(cli: Client, opts: NodeOptions) -> Result<Topologies> {
    let node_api: Api<Node> = Api::all(cli.clone());

    let label_selector = opts.selectors();
    let params = ListParams::default().labels(&label_selector);
    let nodes = only_topology_labels(node_api.list(&params).await?);

    let regions = nodes2region(nodes, HashSet::new());
    let topology = Topology::new(regions);
    let topologies = Topologies(BTreeSet::from([topology]));

    Ok(topologies)
}

#[cfg(test)]
mod tests {
    use kube::api::{ListMeta, ObjectList, TypeMeta};
    use serde::Deserialize;

    use super::*;

    use futures::pin_mut;
    use http::{Request, Response};
    use hyper::Body;
    use tower_test::mock;

    // ref test_mock https://github.com/kube-rs/kube/blob/main/kube-client/src/client/mod.rs
    macro_rules! create_objects {
        ($handle:expr, $file:expr, $return_type:ty) => {
            let (_, send) = $handle.next_request().await.unwrap();
            let yaml = include_str!($file);
            let items = serde_yaml::Deserializer::from_str(yaml)
                .flat_map(<$return_type>::deserialize)
                .collect::<Vec<_>>();
            let types: TypeMeta = TypeMeta::list::<$return_type>();
            let metadata: ListMeta = Default::default();

            let list = ObjectList {
                types,
                metadata,
                items,
            };
            send.send_response(Response::builder().body(Body::from(serde_json::to_vec(&list)?))?);
        };
    }

    #[tokio::test]
    async fn node_ok() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/case1_node.yaml", Node);
            Ok(())
        });
        let cli = Client::new(mock_service, "default");
        let opts = NodeOptions { selector: None };

        let topologies = node(cli, opts).await?;
        spawned.await??;

        assert!(topologies.exists());
        assert!(!topologies.has_name());

        for topology in topologies {
            assert!(topology.exists());
            assert_eq!(topology.name(), String::default());
            for region in topology.regions {
                let mut iter = region.zones.into_iter();

                let zone1 = iter.next().unwrap();
                assert_eq!(zone1.name, "asia-northeast1-a");
                assert_eq!(zone1.count, 2);
                assert_eq!(zone1.skew, 1);

                let zone2 = iter.next().unwrap();
                assert_eq!(zone2.name, "asia-northeast1-b");
                assert_eq!(zone2.count, 1);
                assert_eq!(zone2.skew, 0);

                let zone3 = iter.next().unwrap();
                assert_eq!(zone3.name, "asia-northeast1-c");
                assert_eq!(zone3.count, 1);
                assert_eq!(zone3.skew, 0);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn node_notfound() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/case2_node_empty.yaml", Node);
            Ok(())
        });
        let cli = Client::new(mock_service, "default");
        let opts = NodeOptions { selector: None };

        let topologies = node(cli, opts).await?;
        spawned.await??;

        assert!(!topologies.exists());

        Ok(())
    }

    #[tokio::test]
    async fn pod_ok() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/case3_node.yaml", Node);
            create_objects!(handle, "../tests/case3_pod.yaml", Pod);
            Ok(())
        });

        let ns = "default";
        let cli = Client::new(mock_service, ns);
        let opts = PodOptions {
            namespace: Some(ns.to_owned()),
            selector: None,
        };

        let topologies = pod(cli, opts).await?;

        assert!(topologies.exists());
        assert!(topologies.has_name());

        let mut topologies_iter = topologies.into_iter();

        // apps/v1/daemonset/node-exporter
        // ====================================================================
        let topology1 = topologies_iter.next().unwrap();
        assert_eq!(topology1.name(), "apps/v1/daemonset/node-exporter");

        let mut iter = topology1.regions.into_iter();
        let region1 = iter.next().unwrap();
        assert_eq!(region1.name, "asia-northeast1");

        let mut iter = region1.zones.into_iter();
        let zone1 = iter.next().unwrap();
        assert_eq!(zone1.name, "asia-northeast1-a");
        assert_eq!(zone1.count, 1);
        assert_eq!(zone1.skew, 0);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-b");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 0);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-c");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 0);

        // apps/v1/replicaset/nginx-56fcdc489c
        // ====================================================================
        let topology2 = topologies_iter.next().unwrap();
        assert_eq!(topology2.name(), "apps/v1/replicaset/nginx-56fcdc489c");

        let mut iter = topology2.regions.into_iter();
        let region1 = iter.next().unwrap();
        assert_eq!(region1.name, "asia-northeast1");

        let mut iter = region1.zones.into_iter();
        let zone1 = iter.next().unwrap();
        assert_eq!(zone1.name, "asia-northeast1-a");
        assert_eq!(zone1.count, 3);
        assert_eq!(zone1.skew, 2);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-b");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 0);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-c");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 0);

        // apps/v1/statefulset/mysql
        // ====================================================================
        let topology3 = topologies_iter.next().unwrap();
        assert_eq!(topology3.name(), "apps/v1/statefulset/mysql");

        let mut iter = topology3.regions.into_iter();
        let region1 = iter.next().unwrap();
        assert_eq!(region1.name, "asia-northeast1");

        let mut iter = region1.zones.into_iter();
        let zone1 = iter.next().unwrap();
        assert_eq!(zone1.name, "asia-northeast1-a");
        assert_eq!(zone1.count, 1);
        assert_eq!(zone1.skew, 1);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-b");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 1);

        // batch/v1/job/batch
        // ====================================================================
        let topology3 = topologies_iter.next().unwrap();
        assert_eq!(topology3.name(), "batch/v1/job/batch");

        let mut iter = topology3.regions.into_iter();
        let region1 = iter.next().unwrap();
        assert_eq!(region1.name, "asia-northeast1");

        let mut iter = region1.zones.into_iter();
        let zone1 = iter.next().unwrap();
        assert_eq!(zone1.name, "asia-northeast1-a");
        assert_eq!(zone1.count, 2);
        assert_eq!(zone1.skew, 2);

        let zone2 = iter.next().unwrap();
        assert_eq!(zone2.name, "asia-northeast1-b");
        assert_eq!(zone2.count, 1);
        assert_eq!(zone2.skew, 1);

        spawned.await??;

        Ok(())
    }

    #[tokio::test]
    async fn pod_notfound() -> Result<()> {
        let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
        let spawned = tokio::spawn(async move {
            pin_mut!(handle);
            create_objects!(handle, "../tests/case4_node.yaml", Node);
            create_objects!(handle, "../tests/case4_pod_empty.yaml", Pod);
            Ok(())
        });

        let namespace = String::from("default");
        let cli = Client::new(mock_service, &namespace);
        let opts = PodOptions {
            namespace: Some(namespace),
            selector: None,
        };

        let topologies = pod(cli, opts).await?;
        spawned.await??;
        assert!(!topologies.exists());

        Ok(())
    }
}
