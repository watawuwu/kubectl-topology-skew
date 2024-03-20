use crate::{arg::OutputFormat, TopologyTable, TopologyTables};
use anyhow::*;
use tabled::{
    settings::{object::Rows, Alignment, Border, Panel, Style},
    Table,
};

pub fn out(topologies: TopologyTables, format: OutputFormat) -> Result<String> {
    let buf = match format {
        OutputFormat::Text => text(topologies),
        OutputFormat::Json => json(topologies)?,
        OutputFormat::Yaml => yaml(topologies)?,
    };
    Ok(buf)
}

pub fn text(topology_tables: TopologyTables) -> String {
    let header_border = Border::full(' ', '─', ' ', ' ', ' ', ' ', '─', '─');

    let collect_view_table = |mut outputs: Vec<String>, topology_table: TopologyTable| {
        let mut table = Table::new(topology_table.topologies);
        table.with(Style::blank());

        if let Some(title) = topology_table.header {
            table
                .with(Panel::header(title))
                .modify(Rows::first(), Alignment::center())
                .modify(Rows::first(), header_border);
        }

        outputs.push(table.to_string());
        outputs
    };

    let outputs = topology_tables
        .into_iter()
        .fold(Vec::new(), collect_view_table);

    outputs.join("\n")
}

fn json(topologies: TopologyTables) -> Result<String> {
    Ok(serde_json::to_string_pretty(&topologies)?)
}

fn yaml(topologies: TopologyTables) -> Result<String> {
    Ok(serde_yaml::to_string(&topologies)?)
}
