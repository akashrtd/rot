use rot_core::RuntimeSecurityConfig;

pub async fn run(
    name: Option<&str>,
    runtime_security: RuntimeSecurityConfig,
) -> anyhow::Result<()> {
    let (_, tools) = super::load_tool_registry(runtime_security).await?;

    match name {
        Some(name) => print_tool_detail(&tools, name)?,
        None => print_tool_list(&tools),
    }

    Ok(())
}

fn print_tool_list(tools: &rot_tools::ToolRegistry) {
    let mut names = tools.names();
    names.sort();

    println!("Loaded tools ({}):", names.len());
    for name in names {
        let kind = tool_kind(&name);
        let description = tools
            .get(&name)
            .map(|tool| tool.description().to_string())
            .unwrap_or_else(|| "unknown tool".to_string());
        println!("{name} [{kind}] - {description}");
    }
}

fn print_tool_detail(tools: &rot_tools::ToolRegistry, name: &str) -> anyhow::Result<()> {
    let tool = tools
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
    let schema = serde_json::to_string_pretty(&tool.parameters_schema())?;

    println!("name: {}", tool.name());
    println!("kind: {}", tool_kind(tool.name()));
    println!("label: {}", tool.label());
    println!("description: {}", tool.description());
    println!("parameters:");
    println!("{schema}");

    Ok(())
}

fn tool_kind(name: &str) -> &'static str {
    if name.starts_with("mcp__") {
        "mcp"
    } else if matches!(
        name,
        "read" | "write" | "edit" | "bash" | "glob" | "grep" | "task" | "webfetch"
    ) {
        "builtin"
    } else {
        "custom"
    }
}

#[cfg(test)]
mod tests {
    use super::tool_kind;

    #[test]
    fn test_tool_kind_builtin() {
        assert_eq!(tool_kind("read"), "builtin");
    }

    #[test]
    fn test_tool_kind_mcp() {
        assert_eq!(tool_kind("mcp__fs__read_file"), "mcp");
    }

    #[test]
    fn test_tool_kind_custom() {
        assert_eq!(tool_kind("echo_args"), "custom");
    }
}
