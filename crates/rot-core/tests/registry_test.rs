use rot_core::AgentRegistry;

#[test]
fn test_default_agent() {
    let agent = AgentRegistry::default_agent();
    assert_eq!(agent.name, "default");
    assert!(!agent.system_prompt.is_empty());
}

#[test]
fn test_builtins_not_empty() {
    let agents = AgentRegistry::builtins();
    assert!(!agents.is_empty());
}

#[test]
fn test_builtins_contains_default() {
    let agents = AgentRegistry::builtins();
    assert!(agents.iter().any(|a| a.name == "default"));
}

#[test]
fn test_builtins_contains_plan() {
    let agents = AgentRegistry::builtins();
    assert!(agents.iter().any(|a| a.name == "plan"));
}

#[test]
fn test_builtins_contains_build() {
    let agents = AgentRegistry::builtins();
    assert!(agents.iter().any(|a| a.name == "build"));
}

#[test]
fn test_builtins_contains_explore() {
    let agents = AgentRegistry::builtins();
    assert!(agents.iter().any(|a| a.name == "explore"));
}

#[test]
fn test_builtins_contains_review() {
    let agents = AgentRegistry::builtins();
    assert!(agents.iter().any(|a| a.name == "review"));
}

#[test]
fn test_get_existing_agent() {
    let agent = AgentRegistry::get("default");
    assert!(agent.is_some());
    assert_eq!(agent.unwrap().name, "default");
}

#[test]
fn test_get_existing_agent_case_insensitive() {
    let agent = AgentRegistry::get("DEFAULT");
    assert!(agent.is_some());
    
    let agent = AgentRegistry::get("Plan");
    assert!(agent.is_some());
}

#[test]
fn test_get_nonexistent_agent() {
    let agent = AgentRegistry::get("nonexistent");
    assert!(agent.is_none());
}

#[test]
fn test_resolve_none_returns_default() {
    let agent = AgentRegistry::resolve(None).unwrap();
    assert_eq!(agent.name, "default");
}

#[test]
fn test_resolve_some_existing() {
    let agent = AgentRegistry::resolve(Some("plan")).unwrap();
    assert_eq!(agent.name, "plan");
}

#[test]
fn test_resolve_some_nonexistent() {
    let result = AgentRegistry::resolve(Some("nonexistent"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unknown agent"));
    assert!(err.contains("nonexistent"));
}

#[test]
fn test_primary_agents() {
    let primary = AgentRegistry::primary_agents();
    assert!(!primary.is_empty());
    assert!(primary.iter().all(|a| a.is_primary()));
}

#[test]
fn test_primary_agents_contains_default() {
    let primary = AgentRegistry::primary_agents();
    assert!(primary.iter().any(|a| a.name == "default"));
}

#[test]
fn test_primary_agents_contains_plan() {
    let primary = AgentRegistry::primary_agents();
    assert!(primary.iter().any(|a| a.name == "plan"));
}

#[test]
fn test_primary_agents_excludes_subagents() {
    let primary = AgentRegistry::primary_agents();
    assert!(!primary.iter().any(|a| a.name == "review"));
    assert!(!primary.iter().any(|a| a.name == "explore"));
}

#[test]
fn test_default_chat_system_prompt() {
    let prompt = AgentRegistry::default_chat_system_prompt();
    assert!(!prompt.is_empty());
    assert!(prompt.contains("rot"));
}

#[test]
fn test_agent_is_subagent() {
    let default = AgentRegistry::get("default").unwrap();
    assert!(!default.is_subagent());
    
    let review = AgentRegistry::get("review").unwrap();
    assert!(review.is_subagent());
    
    let explore = AgentRegistry::get("explore").unwrap();
    assert!(explore.is_subagent());
}

#[test]
fn test_agent_display_name() {
    let agent = AgentRegistry::get("default").unwrap();
    assert_eq!(agent.display_name, "Default");
    
    let agent = AgentRegistry::get("plan").unwrap();
    assert_eq!(agent.display_name, "Plan");
}
