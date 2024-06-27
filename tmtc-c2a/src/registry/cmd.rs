use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

use anyhow::{anyhow, Result};
use gaia_ccsds_c2a::access::cmd::schema::{from_tlmcmddb, CommandSchema, Metadata};
use itertools::Itertools;

use crate::proto::tmtc_generic_c2a as proto;
use crate::satconfig;

struct TcoName {
    prefix: String,
    component: String,
    command: String,
}

impl Display for TcoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.prefix, self.component, self.command)
    }
}

impl FromStr for TcoName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let prefix = parts.next().ok_or(())?;
        let component = parts.next().ok_or(())?;
        let command = parts.next().ok_or(())?;
        if parts.next().is_some() {
            return Err(());
        }
        Ok(Self {
            prefix: prefix.to_string(),
            component: component.to_string(),
            command: command.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FatCommandSchema<'a> {
    pub apid: u16,
    pub command_id: u16,
    pub destination_type: u8,
    pub execution_type: u8,
    pub has_time_indicator: bool,
    pub schema: &'a CommandSchema,
}

#[derive(Debug, Clone)]
struct CommandSchemaWithId {
    apid: u16,
    command_id: u16,
    schema: CommandSchema,
}

#[derive(Debug, Clone)]
pub struct Registry {
    prefix_map: satconfig::CommandPrefixMap,
    schema_map: HashMap<(String, String), (Metadata, CommandSchemaWithId)>,
}

impl Registry {
    pub fn build_command_prefix_schema_map(&self) -> HashMap<String, proto::CommandPrefixSchema> {
        self.prefix_map
            .iter()
            .map(|(prefix, subsystem_map)| {
                let prefix = prefix.to_string();
                let subsystems = subsystem_map
                    .iter()
                    .map(|(component_name, subsystem)| {
                        let component_name = component_name.to_string();
                        let command_subsystem_schema = proto::CommandSubsystemSchema {
                            metadata: Some(proto::CommandSubsystemSchemaMetadata {
                                destination_type: subsystem.destination_type as u32,
                                execution_type: subsystem.execution_type as u32,
                            }),
                            has_time_indicator: subsystem.has_time_indicator,
                        };
                        (component_name, command_subsystem_schema)
                    })
                    .collect();
                let command_prefix_schema = proto::CommandPrefixSchema {
                    metadata: Some(proto::CommandPrefixSchemaMetadata {}),
                    subsystems,
                };
                (prefix, command_prefix_schema)
            })
            .collect()
    }

    pub fn build_command_component_schema_map(
        &self,
    ) -> HashMap<String, proto::CommandComponentSchema> {
        self.schema_map
            .iter()
            .sorted_by_key(|&((component_name, _), _)| component_name)
            .group_by(|&((component_name, _), (_, schema_with_id))| {
                (component_name, schema_with_id.apid)
            })
            .into_iter()
            .map(|((component_name, apid), group)| {
                let command_schema_map = group
                    .map(|((_, command_name), (metadata, schema_with_id))| {
                        let trailer_parameter = if schema_with_id.schema.has_trailer_parameter {
                            Some(proto::CommandParameterSchema {
                                metadata: Some(proto::CommandParameterSchemaMetadata {
                                    description: metadata.description.clone(),
                                }),
                                data_type: proto::CommandParameterDataType::CmdParameterBytes
                                    .into(),
                            })
                        } else {
                            None
                        };
                        let parameters = schema_with_id
                            .schema
                            .sized_parameters
                            .iter()
                            .map(|param| {
                                let data_type = match param.value {
                                    structpack::NumericField::Integral(_) => {
                                        proto::CommandParameterDataType::CmdParameterInteger
                                    }
                                    structpack::NumericField::Floating(_) => {
                                        proto::CommandParameterDataType::CmdParameterDouble
                                    }
                                };
                                proto::CommandParameterSchema {
                                    metadata: Some(proto::CommandParameterSchemaMetadata {
                                        description: param.description.clone(),
                                    }),
                                    data_type: data_type.into(),
                                }
                            })
                            .chain(trailer_parameter)
                            .collect();
                        let command_name = command_name.to_string();
                        let command_schema = proto::CommandSchema {
                            metadata: Some(proto::CommandSchemaMetadata {
                                id: schema_with_id.command_id as u32,
                                description: metadata.description.clone(),
                            }),
                            parameters,
                        };
                        (command_name, command_schema)
                    })
                    .collect();
                let command_component_schema = proto::CommandComponentSchema {
                    metadata: Some(proto::CommandComponentSchemaMetadata { apid: apid as u32 }),
                    commands: command_schema_map,
                };
                (component_name.to_string(), command_component_schema)
            })
            .collect()
    }

    pub fn lookup(&self, tco_name: &str) -> Option<FatCommandSchema> {
        let TcoName {
            prefix,
            component,
            command,
        } = tco_name.parse().ok()?;
        let satconfig::CommandSubsystem {
            has_time_indicator,
            destination_type,
            execution_type,
        } = self.prefix_map.get(&prefix)?.get(&component)?;
        let (
            _,
            CommandSchemaWithId {
                apid,
                command_id,
                schema,
            },
        ) = self.schema_map.get(&(component, command))?;
        Some(FatCommandSchema {
            apid: *apid,
            command_id: *command_id,
            destination_type: *destination_type,
            execution_type: *execution_type,
            has_time_indicator: *has_time_indicator,
            schema,
        })
    }

    pub fn from_tlmcmddb_with_satconfig(
        db: &tlmcmddb::Database,
        apid_map: &HashMap<String, u16>,
        prefix_map: satconfig::CommandPrefixMap,
    ) -> Result<Self> {
        let schema_map = from_tlmcmddb(db)
            .flatten()
            .map(|schema| {
                let (metadata, schema) = schema?;
                let component = metadata.component_name.clone();
                let cmddb_name = metadata.command_name.clone();
                let apid = *apid_map
                    .get(&component)
                    .ok_or_else(|| anyhow!("APID is not defined for {component}"))?;
                let schema_with_id = CommandSchemaWithId {
                    apid,
                    command_id: metadata.cmd_id,
                    schema,
                };
                Ok(((component, cmddb_name), (metadata, schema_with_id)))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            schema_map,
            prefix_map,
        })
    }
}
