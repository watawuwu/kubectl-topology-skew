use ::kube::{
    api::{Api, ListParams, ObjectList, ResourceExt},
    config::KubeConfigOptions,
    Client,
};
use anyhow::*;
use itertools::Itertools;
use k8s_openapi::api::core::v1::{Node, Pod};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{Display, Formatter},
    sync::RwLock,
};

pub const REGION_LABEL: &str = "topology.kubernetes.io/region";
pub const ZONE_LABEL: &str = "topology.kubernetes.io/zone";

#[derive(Debug, Clone)]
pub struct Label(pub String, pub String);
impl Display for Label {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}={}", &self.0, &self.1)
    }
}

pub trait LabelSelector {
    fn selector(&self) -> String;
}

impl LabelSelector for Vec<Label> {
    fn selector(&self) -> String {
        self.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub trait OwnerRefAbbreviation {
    fn owner_abbr(&self) -> String;
}

impl OwnerRefAbbreviation for Pod {
    fn owner_abbr(&self) -> String {
        self.owner_references()
            .iter()
            .map(|or| format!("{}/{}/{}", or.api_version, or.kind.to_lowercase(), or.name))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug)]
pub struct CachedNodeApi {
    api: Api<Node>,
    cached: RwLock<HashMap<String, Node>>,
}

impl CachedNodeApi {
    pub async fn try_from(cli: Client) -> Result<Self> {
        let api = Api::all(cli.clone());
        let lp = ListParams::default();
        let cached = api
            .list(&lp)
            .await?
            .into_iter()
            .map(|node: Node| (node.name_any(), node))
            .collect::<HashMap<_, _>>();

        Ok(Self {
            api,
            cached: RwLock::new(cached),
        })
    }

    pub fn all_zones(&self) -> HashSet<String> {
        let cached = self.cached.read().unwrap();
        cached
            .iter()
            .filter(|(_, node)| node.labels().get(ZONE_LABEL).is_some())
            .fold(HashSet::new(), |mut acc, (_, node)| {
                let zone = node.labels().get(ZONE_LABEL).unwrap();
                acc.insert(zone.to_owned());
                acc
            })
    }

    pub async fn get(&self, node_name: &str) -> Option<Node> {
        if let Some(n) = self.cached.read().unwrap().get(node_name) {
            return Some(n.clone());
        }

        let Some(node) = self.api.get(node_name).await.ok() else {
            return None;
        };

        self.cached
            .write()
            .unwrap()
            .insert(node_name.to_string(), node);

        self.cached.read().unwrap().get(node_name).map(Clone::clone)
    }
}

pub async fn kube_client(
    context: Option<String>,
    cluster: Option<String>,
    user: Option<String>,
) -> Result<Client> {
    let config = kube::Config::from_kubeconfig(&KubeConfigOptions {
        context,
        cluster,
        user,
    })
    .await?;

    Ok(Client::try_from(config)?)
}

pub fn only_running(pods: ObjectList<Pod>) -> Vec<Pod> {
    pods.into_iter()
        .filter(|pod| {
            pod.status
                .as_ref()
                .and_then(|status| status.phase.as_ref().map(|phase| phase == "Running"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>()
}

pub fn group_by_owner_abbr(pods: Vec<Pod>) -> BTreeMap<Option<String>, Vec<Pod>> {
    pods.into_iter()
        .map(|pod| (pod.owner_abbr(), pod))
        .into_group_map()
        .into_iter()
        .map(|(owner_abbr, pods)| (Some(owner_abbr), pods))
        .collect::<BTreeMap<_, _>>()
}

pub fn only_topology_labels(nodes: ObjectList<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .filter(|node| node.labels().contains_key(REGION_LABEL))
        .filter(|node| node.labels().contains_key(ZONE_LABEL))
        .collect::<Vec<_>>()
}

pub fn region(node: &Node) -> String {
    node.labels()
        .get(REGION_LABEL)
        .map(String::from)
        .unwrap_or_default()
}

pub fn zone(node: &Node) -> String {
    node.labels()
        .get(ZONE_LABEL)
        .map(String::from)
        .unwrap_or_default()
}
