use anyhow::*;
use comfy_table::*;
use termtree::Tree;

use crate::{arg::OutputFormat, Topologies};

pub fn out(topologies: Topologies, format: OutputFormat) -> Result<String> {
    if !topologies.exists() {
        return Ok(String::from("No resources found."));
    }

    let buf = match format {
        OutputFormat::Text => text(topologies),
        OutputFormat::Json => json(topologies)?,
        OutputFormat::Yaml => yaml(topologies)?,
        OutputFormat::Tree => tree(topologies),
    };
    Ok(buf)
}

pub fn text(topologies: Topologies) -> String {
    let has_name: bool = topologies.has_name();

    let mut headers = vec!["REGION", "ZONE", "COUNT", "SKEW"];
    if has_name {
        headers.insert(0, "NAME");
    }

    let mut table = Table::new();
    table
        .load_preset(comfy_table::presets::NOTHING)
        .set_header(headers);

    for topology in topologies {
        let name = topology.name();
        for region in topology.regions {
            for zone in region.zones {
                let count = zone.count.to_string();
                let skew = zone.skew.to_string();
                let mut row = vec![&region.name, &zone.name, &count, &skew];
                if has_name {
                    row.insert(0, &name)
                }
                table.add_row(row);
            }
        }
    }

    table.to_string()
}

fn json(topologies: Topologies) -> Result<String> {
    Ok(serde_json::to_string_pretty(&topologies)?)
}

fn yaml(topologies: Topologies) -> Result<String> {
    Ok(serde_yaml::to_string(&topologies)?)
}

fn tree(topologies: Topologies) -> String {
    let has_name: bool = topologies.has_name();

    let mut root_tree = Tree::new(String::from("."));
    for topology in topologies {
        let mut topology_tree = Tree::new(topology.name());
        for region in topology.regions {
            let mut region_tree = Tree::new(region.name.to_owned());
            for zone in region.zones {
                region_tree.push(zone.to_string());
            }
            if has_name {
                topology_tree.push(region_tree);
            } else {
                root_tree.push(region_tree);
            }
        }
        if has_name {
            root_tree.push(topology_tree);
        }
    }

    root_tree.to_string()
}
