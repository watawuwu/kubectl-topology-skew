use crate::{nodes_by, pods_by, spreading_status, CachedNodeApi};
use anyhow::*;
use derive_more::{Constructor, Deref, DerefMut, From, IntoIterator};
use itertools::Itertools;
use kube::Client;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Debug;
use tabled::Tabled;

#[derive(Debug, Default, Serialize, PartialEq, PartialOrd, Deref, DerefMut, IntoIterator, From)]
pub struct TopologyTables(BTreeSet<TopologyTable>);

#[derive(Debug, Default, Serialize, PartialEq, Eq, PartialOrd, Ord, Constructor)]
pub struct TopologyTable {
    pub topologies: Topologies,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
}

impl TopologyTable {
    pub fn create(
        topology_values: Vec<String>,
        domains: &HashSet<String>,
        header: Option<String>,
    ) -> TopologyTable {
        let topologies = Topologies::create_with_skew_calculation(topology_values, domains);

        TopologyTable::new(topologies, header)
    }
}

#[derive(Debug, Default, Serialize, PartialEq, Eq, PartialOrd, Ord, IntoIterator)]
pub struct Topologies(BTreeSet<Topology>);

impl Topologies {
    pub fn create_with_skew_calculation(
        topology_values: Vec<String>,
        domains: &HashSet<String>,
    ) -> Self {
        let counts_by_value = topology_values.into_iter().counts();

        let mut counts_by_domain = domains
            .iter()
            .map(|name| (name.clone(), 0usize))
            .collect::<HashMap<_, _>>();

        counts_by_domain.extend(counts_by_value);

        // global_minimum is defined in the following documents
        // https://kubernetes.io/docs/concepts/scheduling-eviction/topology-spread-constraints/#spread-constraint-definition
        // > The global minimum is the minimum number of matching Pods in an eligible domain, or zero if the number of eligible domains is less than minDomains.
        let global_minimum = counts_by_domain
            .values()
            .min()
            .map(ToOwned::to_owned)
            .unwrap_or_default();

        let calc = |(key, count): (String, usize)| {
            let skew = count - global_minimum;
            Topology::new(key, count as u32, skew as u32)
        };
        let topologies = counts_by_domain
            .into_iter()
            .map(calc)
            .collect::<BTreeSet<_>>();

        Topologies(topologies)
    }
}

#[derive(Debug, Tabled, Default, Serialize, PartialEq, Eq, PartialOrd, Ord, Constructor)]
#[tabled(rename_all = "UPPERCASE")]
pub struct Topology {
    #[tabled(rename = "TOPOLOGY")]
    pub key: String,
    pub count: u32,
    pub skew: u32,
}

pub async fn topology_table_find_by(
    labels_map: BTreeMap<String, String>,
    namespace: &str,
    topology_key: &str,
    cli: Client,
    use_header: bool,
) -> Result<TopologyTables> {
    let mut tables = TopologyTables::default();
    let node_api = CachedNodeApi::try_from(cli.clone()).await?;

    for (name, labels) in labels_map {
        let pods = pods_by(&[&labels], namespace, cli.clone()).await?;
        let nodes = nodes_by(&pods, &node_api).await?;

        if nodes.is_empty() {
            bail!("No found objects")
        }
        let (topology_values, domains) = spreading_status(&nodes, topology_key, &node_api).await?;
        let header = use_header.then_some(name);
        let table = TopologyTable::create(topology_values, &domains, header);

        tables.insert(table);
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_with_skew_calculation_ok() {
        let domains = HashSet::from([
            String::from("zone-a"),
            String::from("zone-b"),
            String::from("zone-c"),
        ]);

        let table = vec![
            (
                (vec!["zone-a", "zone-b", "zone-c"], &domains), // arg1 topology_values
                ("zone-a", 1, 0, "zone-b", 1, 0, "zone-c", 1, 0), // arg2 domains
            ),
            (
                (
                    vec!["zone-a", "zone-b", "zone-c", "zone-a", "zone-a"],
                    &domains,
                ),
                ("zone-a", 3, 2, "zone-b", 1, 0, "zone-c", 1, 0),
            ),
            (
                (
                    vec!["zone-c", "zone-a", "zone-a", "zone-b", "zone-a", "zone-b"],
                    &domains,
                ),
                ("zone-a", 3, 2, "zone-b", 2, 1, "zone-c", 1, 0),
            ),
            (
                (vec!["zone-a"], &domains),
                ("zone-a", 1, 1, "zone-b", 0, 0, "zone-c", 0, 0),
            ),
            (
                (vec!["zone-a", "zone-a", "zone-a", "zone-a"], &domains),
                ("zone-a", 4, 4, "zone-b", 0, 0, "zone-c", 0, 0),
            ),
            (
                (vec!["zone-a", "zone-b"], &domains),
                ("zone-a", 1, 1, "zone-b", 1, 1, "zone-c", 0, 0),
            ),
            (
                (Vec::new(), &domains),
                ("zone-a", 0, 0, "zone-b", 0, 0, "zone-c", 0, 0),
            ),
            (
                (Vec::new(), &domains),
                ("zone-a", 0, 0, "zone-b", 0, 0, "zone-c", 0, 0),
            ),
        ];

        for (
            (topology_values, domains),
            (e1_key, e1_count, e1_skew, e2_key, e2_count, e2_skew, e3_key, e3_count, e3_skew),
        ) in table
        {
            let topology_values = topology_values
                .into_iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();

            let topologies = Topologies::create_with_skew_calculation(topology_values, domains);
            let mut iter = topologies.into_iter();

            let topology = iter.next().unwrap();
            assert_eq!(topology.key, e1_key.to_owned());
            assert_eq!(topology.count, e1_count);
            assert_eq!(topology.skew, e1_skew);

            let topology = iter.next().unwrap();
            assert_eq!(topology.key, e2_key.to_owned());
            assert_eq!(topology.count, e2_count);
            assert_eq!(topology.skew, e2_skew);

            let topology = iter.next().unwrap();
            assert_eq!(topology.key, e3_key.to_owned());
            assert_eq!(topology.count, e3_count);
            assert_eq!(topology.skew, e3_skew);
        }
    }
}
