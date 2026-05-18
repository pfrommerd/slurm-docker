use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dockerfile {
    instructions: Vec<Instruction>,
}

impl Dockerfile {
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    pub fn parse(input: &str) -> Result<Self> {
        let mut instructions = Vec::new();
        for line in logical_lines(input) {
            if line.trim().is_empty() {
                continue;
            }
            instructions.push(Instruction::parse(&line)?);
        }
        Ok(Self::new(group_stages(instructions)))
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}

impl std::str::FromStr for Dockerfile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    From(FromInstruction),
    Run(ShellOrExec),
    Cmd(ShellOrExec),
    Label(Vec<KeyValue>),
    Maintainer(String),
    Expose(Vec<Port>),
    Env(Vec<KeyValue>),
    Add(CopyInstruction),
    Copy(CopyInstruction),
    Entrypoint(ShellOrExec),
    Volume(Vec<String>),
    User(String),
    Workdir(String),
    Arg {
        name: String,
        default: Option<String>,
    },
    Onbuild(Box<Instruction>),
    Stopsignal(String),
    Healthcheck(Healthcheck),
    Shell(Vec<String>),
    ParserDirective {
        key: String,
        value: String,
    },
    Comment(String),
    Stage {
        from: FromInstruction,
        dockerfile: Arc<Dockerfile>,
    },
    Unknown {
        keyword: String,
        value: String,
    },
}

impl Instruction {
    pub fn parse(line: &str) -> Result<Self> {
        let trimmed = line.trim();
        if let Some(comment) = trimmed.strip_prefix('#') {
            if let Some((key, value)) = comment.split_once('=') {
                return Ok(Instruction::ParserDirective {
                    key: key.trim().to_ascii_lowercase(),
                    value: value.trim().to_string(),
                });
            }
            return Ok(Instruction::Comment(comment.trim().to_string()));
        }

        let (keyword, value) = split_keyword(trimmed)?;
        match keyword.as_str() {
            "FROM" => Ok(Instruction::From(parse_from(value)?)),
            "RUN" => Ok(Instruction::Run(ShellOrExec::parse(value)?)),
            "CMD" => Ok(Instruction::Cmd(ShellOrExec::parse(value)?)),
            "LABEL" => Ok(Instruction::Label(parse_key_values(value)?)),
            "MAINTAINER" => Ok(Instruction::Maintainer(value.trim().to_string())),
            "EXPOSE" => Ok(Instruction::Expose(parse_ports(value))),
            "ENV" => Ok(Instruction::Env(parse_key_values(value)?)),
            "ADD" => Ok(Instruction::Add(parse_copy_like(value)?)),
            "COPY" => Ok(Instruction::Copy(parse_copy_like(value)?)),
            "ENTRYPOINT" => Ok(Instruction::Entrypoint(ShellOrExec::parse(value)?)),
            "VOLUME" => Ok(Instruction::Volume(parse_string_or_json_array(value)?)),
            "USER" => Ok(Instruction::User(value.trim().to_string())),
            "WORKDIR" => Ok(Instruction::Workdir(value.trim().to_string())),
            "ARG" => {
                let (name, default) = value
                    .split_once('=')
                    .map(|(name, default)| (name.trim(), Some(default.trim().to_string())))
                    .unwrap_or((value.trim(), None));
                Ok(Instruction::Arg {
                    name: name.to_string(),
                    default,
                })
            }
            "ONBUILD" => Ok(Instruction::Onbuild(Box::new(Instruction::parse(value)?))),
            "STOPSIGNAL" => Ok(Instruction::Stopsignal(value.trim().to_string())),
            "HEALTHCHECK" => Ok(Instruction::Healthcheck(parse_healthcheck(value)?)),
            "SHELL" => Ok(Instruction::Shell(parse_json_array(value)?)),
            _ => Ok(Instruction::Unknown {
                keyword,
                value: value.trim().to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FromInstruction {
    pub image: String,
    pub platform: Option<String>,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellOrExec {
    Shell(String),
    Exec(Vec<String>),
}

impl ShellOrExec {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.starts_with('[') {
            Ok(ShellOrExec::Exec(parse_json_array(value)?))
        } else {
            Ok(ShellOrExec::Shell(value.to_string()))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub port: String,
    pub protocol: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyInstruction {
    pub flags: Vec<String>,
    pub from: Option<String>,
    pub sources: Vec<String>,
    pub destination: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Healthcheck {
    None,
    Command {
        options: Vec<String>,
        command: ShellOrExec,
    },
}

fn split_keyword(line: &str) -> Result<(String, &str)> {
    let Some((keyword, rest)) = line.split_once(char::is_whitespace) else {
        return Err(Error::DockerfileParse(format!(
            "instruction has no body: {line}"
        )));
    };
    Ok((keyword.to_ascii_uppercase(), rest.trim()))
}

fn logical_lines(input: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw in input.lines() {
        let trimmed_end = raw.trim_end();
        if let Some(prefix) = trimmed_end.strip_suffix('\\') {
            current.push_str(prefix);
            current.push(' ');
        } else {
            current.push_str(trimmed_end);
            lines.push(current.trim().to_string());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        lines.push(current.trim().to_string());
    }
    lines
}

fn group_stages(instructions: Vec<Instruction>) -> Vec<Instruction> {
    if instructions
        .iter()
        .filter(|instruction| matches!(instruction, Instruction::From(_)))
        .count()
        <= 1
    {
        return instructions;
    }

    let mut grouped = Vec::new();
    let mut current_from = None;
    let mut current_body = Vec::new();

    for instruction in instructions {
        match instruction {
            Instruction::From(from) => {
                if let Some(from) = current_from.replace(from) {
                    grouped.push(Instruction::Stage {
                        from,
                        dockerfile: Arc::new(Dockerfile::new(std::mem::take(&mut current_body))),
                    });
                }
            }
            other => current_body.push(other),
        }
    }

    if let Some(from) = current_from {
        grouped.push(Instruction::Stage {
            from,
            dockerfile: Arc::new(Dockerfile::new(current_body)),
        });
    }

    grouped
}

fn parse_from(value: &str) -> Result<FromInstruction> {
    let mut platform = None;
    let mut parts = shell_words::split(value)
        .map_err(|err| Error::DockerfileParse(format!("invalid FROM: {err}")))?
        .into_iter()
        .peekable();

    while let Some(part) = parts.peek() {
        if let Some(value) = part.strip_prefix("--platform=") {
            platform = Some(value.to_string());
            parts.next();
        } else {
            break;
        }
    }

    let image = parts
        .next()
        .ok_or_else(|| Error::DockerfileParse("FROM requires an image".to_string()))?;
    let mut alias = None;
    if let Some(next) = parts.next() {
        if !next.eq_ignore_ascii_case("AS") {
            return Err(Error::DockerfileParse(format!(
                "expected AS in FROM, got {next}"
            )));
        }
        alias = parts.next();
    }

    Ok(FromInstruction {
        image,
        platform,
        alias,
    })
}

fn parse_key_values(value: &str) -> Result<Vec<KeyValue>> {
    let fields = shell_words::split(value)
        .map_err(|err| Error::DockerfileParse(format!("invalid key/value list: {err}")))?;
    if fields.is_empty() {
        return Ok(Vec::new());
    }

    if fields.iter().all(|field| field.contains('=')) {
        return fields
            .into_iter()
            .map(|field| {
                let (key, value) = field.split_once('=').expect("checked contains");
                Ok(KeyValue {
                    key: key.to_string(),
                    value: value.to_string(),
                })
            })
            .collect();
    }

    if fields.len() >= 2 {
        return Ok(vec![KeyValue {
            key: fields[0].clone(),
            value: fields[1..].join(" "),
        }]);
    }

    Err(Error::DockerfileParse(format!(
        "expected KEY=VALUE or KEY VALUE, got {value}"
    )))
}

fn parse_ports(value: &str) -> Vec<Port> {
    value
        .split_whitespace()
        .map(|raw| {
            let (port, protocol) = raw
                .split_once('/')
                .map(|(port, protocol)| (port, Some(protocol.to_string())))
                .unwrap_or((raw, None));
            Port {
                port: port.to_string(),
                protocol,
            }
        })
        .collect()
}

fn parse_copy_like(value: &str) -> Result<CopyInstruction> {
    let fields = shell_words::split(value)
        .map_err(|err| Error::DockerfileParse(format!("invalid copy instruction: {err}")))?;
    let mut flags = Vec::new();
    let mut from = None;
    let mut paths = Vec::new();

    for field in fields {
        if field.starts_with("--") && paths.is_empty() {
            if let Some(value) = field.strip_prefix("--from=") {
                from = Some(value.to_string());
            }
            flags.push(field);
        } else {
            paths.push(field);
        }
    }

    if paths.len() < 2 {
        return Err(Error::DockerfileParse(
            "COPY/ADD requires at least one source and a destination".to_string(),
        ));
    }

    let destination = paths.pop().expect("length checked");
    Ok(CopyInstruction {
        flags,
        from,
        sources: paths,
        destination,
    })
}

fn parse_healthcheck(value: &str) -> Result<Healthcheck> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("NONE") {
        return Ok(Healthcheck::None);
    }

    let fields = shell_words::split(value)
        .map_err(|err| Error::DockerfileParse(format!("invalid healthcheck: {err}")))?;
    let cmd_index = fields
        .iter()
        .position(|field| field.eq_ignore_ascii_case("CMD"))
        .ok_or_else(|| Error::DockerfileParse("HEALTHCHECK requires CMD or NONE".to_string()))?;

    let options = fields[..cmd_index].to_vec();
    let command_text = fields[cmd_index + 1..].join(" ");
    Ok(Healthcheck::Command {
        options,
        command: ShellOrExec::parse(&command_text)?,
    })
}

fn parse_string_or_json_array(value: &str) -> Result<Vec<String>> {
    let value = value.trim();
    if value.starts_with('[') {
        parse_json_array(value)
    } else {
        shell_words::split(value)
            .map_err(|err| Error::DockerfileParse(format!("invalid string list: {err}")))
    }
}

fn parse_json_array(value: &str) -> Result<Vec<String>> {
    serde_json::from_str(value).map_err(|err| Error::DockerfileParse(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_dockerfile() {
        let dockerfile = Dockerfile::parse(
            r#"
            FROM ubuntu:24.04
            ENV FOO=bar BAZ="qux quux"
            RUN echo hello
            COPY --from=builder src/ /app/
            "#,
        )
        .unwrap();

        assert_eq!(dockerfile.instructions.len(), 4);
        assert!(matches!(dockerfile.instructions[0], Instruction::From(_)));
        assert!(matches!(dockerfile.instructions[2], Instruction::Run(_)));
    }

    #[test]
    fn groups_multi_stage_builds() {
        let dockerfile = Dockerfile::parse(
            r#"
            FROM rust:1 AS builder
            RUN cargo build
            FROM debian:stable
            COPY --from=builder /target/app /app
            "#,
        )
        .unwrap();

        assert_eq!(dockerfile.instructions.len(), 2);
        let Instruction::Stage { from, dockerfile } = &dockerfile.instructions[0] else {
            panic!("expected first instruction to be a stage");
        };
        assert_eq!(from.alias.as_deref(), Some("builder"));
        assert_eq!(dockerfile.instructions.len(), 1);
    }
}
