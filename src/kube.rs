use ::kube::{
    api::{Api, ListParams},
    config::KubeConfigOptions,
    Client, Resource, ResourceExt,
};
use anyhow::*;
use futures::future;
use k8s_openapi::{
    api::core::v1::{Node, NodeStatus, Pod, PodStatus},
    NamespaceResourceScope,
};
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, fmt::Debug};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
    sync::RwLock,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Label(pub String, pub String);
impl Display for Label {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}={}", &self.0, &self.1)
    }
}

impl From<(&str, &str)> for Label {
    fn from(item: (&str, &str)) -> Self {
        Label(item.0.to_owned(), item.1.to_owned())
    }
}

pub trait LabelSelector {
    fn selector(&self) -> String;

    fn labels(&self) -> BTreeMap<String, String>;
}

impl LabelSelector for Vec<Label> {
    fn selector(&self) -> String {
        self.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn labels(&self) -> BTreeMap<String, String> {
        self.iter()
            .map(|label| (label.0.to_string(), label.1.to_string()))
            .collect::<BTreeMap<_, _>>()
    }
}

#[derive(Debug)]
pub struct CachedNodeApi {
    // Command line is short-lived and not reacquired
    cached: RwLock<HashMap<String, Node>>,
}

impl CachedNodeApi {
    pub async fn try_from(cli: Client) -> Result<Self> {
        let api = Api::all(cli.clone());
        let lp = ListParams::default();
        let cached = api.list(&lp).await?;

        let cached = cached
            .into_iter()
            .map(|node: Node| (node.name_any(), node))
            .collect::<HashMap<_, _>>();

        Ok(Self {
            cached: RwLock::new(cached),
        })
    }

    // Domain is defined in the following documents
    //   https://kubernetes.io/docs/concepts/scheduling-eviction/topology-spread-constraints/#spread-constraint-definition
    // A domain is a particular instance of a topology
    pub fn domains(&self, topology_key: &str) -> HashSet<String> {
        let cached = self.cached.read().unwrap();
        let has_topology = |(_, node): &(&String, &Node)| node.labels().get(topology_key).is_some();

        let collect_domains = |mut domains: HashSet<String>, (_, node): (&String, &Node)| {
            let topology = node.labels().get(topology_key).unwrap();
            domains.insert(topology.to_owned());
            domains
        };

        cached
            .iter()
            .filter(has_topology)
            .fold(HashSet::new(), collect_domains)
    }

    // Command line is short-lived and not reacquired
    pub async fn get(&self, node_name: &str) -> Option<Node> {
        self.cached.read().unwrap().get(node_name).cloned()
    }

    pub async fn list(&self, labels: &BTreeMap<String, String>) -> Vec<Node> {
        let find_by_label = |(_, node): (&String, &Node)| {
            labels
                .iter()
                .all(|search_label| {
                    node.labels()
                        .iter()
                        .any(|node_label| search_label == node_label)
                })
                .then_some(node.clone())
        };

        let nodes = self
            .cached
            .read()
            .unwrap()
            .iter()
            .filter_map(find_by_label)
            .collect::<Vec<_>>();

        nodes
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

pub async fn resources<K: Resource>(
    name: Option<&str>,
    namespace: &str,
    label: Option<&str>,
    cli: Client,
) -> Result<Vec<K>>
where
    <K as Resource>::DynamicType: Default,
    K: Resource<Scope = NamespaceResourceScope>,
    K: Clone + DeserializeOwned + Debug,
{
    let api: Api<K> = Api::namespaced(cli, namespace);

    let resources = match (name, label) {
        (Some(n), None) => {
            let resource = api.get(n).await?;
            vec![resource]
        }
        (None, Some(l)) => {
            let params = ListParams::default().labels(l);
            api.list(&params).await?.into_iter().collect::<Vec<_>>()
        }
        _ => {
            let params = ListParams::default();
            api.list(&params).await?.into_iter().collect::<Vec<_>>()
        }
    };

    Ok(resources)
}

pub fn only_pod_running(pods: Vec<Pod>) -> Vec<Pod> {
    let is_running = |status: &PodStatus| status.phase.as_ref().map(|phase| phase == "Running");
    let only_running = |pod: &Pod| pod.status.as_ref().and_then(is_running).unwrap_or(false);

    pods.into_iter().filter(only_running).collect::<Vec<_>>()
}

pub fn only_node_running(nodes: Vec<Node>) -> Vec<Node> {
    let is_ready = |status: &NodeStatus| {
        status.conditions.as_ref().map(|conditions| {
            conditions
                .iter()
                .any(|condi| condi.type_ == "Ready" && condi.status == "True")
        })
    };
    let only_running = |node: &Node| node.status.as_ref().and_then(is_ready).is_some();
    nodes.into_iter().filter(only_running).collect::<Vec<_>>()
}

pub fn topology_values(topology_key: &str, nodes: &[Node]) -> Vec<String> {
    let find_topology_value = |node: &Node| node.labels().get(topology_key).map(String::from);
    nodes
        .iter()
        .filter_map(find_topology_value)
        .collect::<Vec<_>>()
}

pub fn node_names(pods: &[Pod]) -> Vec<&str> {
    pods.iter()
        .filter_map(|pod| pod.spec.as_ref().and_then(|spec| spec.node_name.as_deref()))
        .collect::<Vec<_>>()
}

pub async fn nodes_by(pods: &[Pod], api: &CachedNodeApi) -> Result<Vec<Node>> {
    let node_names = node_names(pods);

    let nodes_fut = node_names
        .iter()
        .map(|node_name| api.get(node_name))
        .collect::<Vec<_>>();

    let nodes = future::join_all(nodes_fut)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let nodes = only_node_running(nodes);

    Ok(nodes)
}

pub async fn pods_by(labels_set: &[&str], namespace: &str, cli: Client) -> Result<Vec<Pod>> {
    let api: Api<Pod> = Api::namespaced(cli, namespace);

    let get_pods = |labels: &&str| {
        let params = ListParams::default().labels(labels);

        let api = &api;
        Ok(async move {
            api.list(&params)
                .await
                .context("Fail to get pods")
                .map(|objs| objs.into_iter().collect::<Vec<_>>())
        })
    };

    let pods_fut = labels_set
        .iter()
        .map(get_pods)
        .collect::<Result<Vec<_>>>()?;

    let pods = future::join_all(pods_fut)
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let pods = only_pod_running(pods);

    Ok(pods)
}

// Retrieve scheduled topology values and domain information to verify spreading status
pub async fn spreading_status(
    nodes: &[Node],
    topology_key: &str,
    api: &CachedNodeApi,
) -> Result<(Vec<String>, HashSet<String>)> {
    let topology_values = topology_values(topology_key, nodes);
    let domains = api.domains(topology_key);
    Ok((topology_values, domains))
}

#[cfg(test)]
pub mod tests {

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

    pub(crate) use create_objects;
}
