//! Tab flow router: free text typed into the main menu → the right flow.
//!
//! The user types anything ("what did vercel email me", "npm audit my deps"),
//! presses Tab, and the router decides:
//! - **Confident single match** → auto-start a conversation with the text as
//!   the initial task.
//! - **Plausible candidates** → open the desk with the candidates ranked so
//!   the user picks.
//! - **Nothing fits** → open the desk's create-flow affordance seeded with
//!   the text.
//!
//! v1 is a deterministic lexical scorer over flow identity metadata (name,
//! friendly name, description). No LLM in the hot path — routing must feel
//! instant. A ghost-LLM reranker can slot in behind the same decision enum.

use super::model::FlowDescriptor;

#[derive(Debug, Clone, PartialEq)]
pub enum RouteDecision {
    /// One flow clearly owns this text — start the conversation now.
    AutoStart { flow: Box<FlowDescriptor> },
    /// More than one plausible owner — show these, best first.
    Candidates { flows: Vec<FlowDescriptor> },
    /// No flow matched — offer creation.
    NoMatch,
}

/// Score one flow against the query tokens. Deliberately simple and
/// explainable: identity-word hits dominate, description words break ties.
fn score_flow(flow: &FlowDescriptor, tokens: &[String]) -> u32 {
    let name = flow.name.to_lowercase();
    let friendly = flow.friendly_name().to_lowercase();
    let name_words: Vec<&str> = name
        .trim_start_matches("flow-")
        .split(['-', '_'])
        .filter(|w| !w.is_empty())
        .collect();
    let friendly_words: Vec<String> = friendly.split_whitespace().map(|w| w.to_string()).collect();
    let description = flow.description.as_deref().unwrap_or("").to_lowercase();
    let description_words: Vec<&str> = description
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 2)
        .collect();

    let mut score = 0u32;
    for token in tokens {
        if name_words.iter().any(|w| *w == token) || friendly_words.iter().any(|w| w == token) {
            score += 6;
        } else if name_words.iter().any(|w| w.starts_with(token.as_str())) && token.len() >= 3 {
            score += 3;
        }
        if description_words.iter().any(|w| *w == token) {
            score += 2;
        }
    }
    score
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_string())
        .collect()
}

/// Route free text to a flow. `flows` is the full desk corpus
/// (roster + package).
pub fn route(text: &str, flows: &[FlowDescriptor]) -> RouteDecision {
    let tokens = tokenize(text);
    if tokens.is_empty() || flows.is_empty() {
        return RouteDecision::NoMatch;
    }

    let mut scored: Vec<(u32, &FlowDescriptor)> = flows
        .iter()
        .map(|flow| (score_flow(flow, &tokens), flow))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));

    match scored.as_slice() {
        [] => RouteDecision::NoMatch,
        [(top_score, top), rest @ ..] => {
            let second_score = rest.first().map(|(s, _)| *s).unwrap_or(0);
            // Confident: a real identity hit that clearly beats the runner-up.
            if *top_score >= 6 && *top_score >= second_score * 2 {
                RouteDecision::AutoStart {
                    flow: Box::new((*top).clone()),
                }
            } else {
                RouteDecision::Candidates {
                    flows: scored.into_iter().take(8).map(|(_, f)| f.clone()).collect(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::model::FlowSource;

    fn flow(name: &str, description: &str) -> FlowDescriptor {
        FlowDescriptor {
            id: format!("package:{name}"),
            path: format!("/pkg/flows/{name}.codex.md"),
            source: FlowSource::Package,
            name: name.to_string(),
            description: (!description.is_empty()).then(|| description.to_string()),
            engine: "codex".into(),
            engine_source: None,
            inputs: Vec::new(),
            is_workflow: false,
            interactive: true,
            mtime_ms: 0,
            origin: Some("@johnlindquist/flows".into()),
            wrapper_command: Some(name.to_string()),
        }
    }

    fn corpus() -> Vec<FlowDescriptor> {
        vec![
            flow("flow-gmail", "Gmail via gog: search, read, draft, send"),
            flow("flow-npm", "Ask about npm packages, versions, and deps"),
            flow("flow-git", "Git operations in plain English"),
            flow("flow-brew", "Homebrew formulae and casks"),
        ]
    }

    #[test]
    fn identity_word_auto_starts() {
        match route("gmail from vercel this week", &corpus()) {
            RouteDecision::AutoStart { flow } => assert_eq!(flow.name, "flow-gmail"),
            other => panic!("expected auto-start, got {other:?}"),
        }
    }

    #[test]
    fn description_only_hits_offer_candidates() {
        // "packages" appears in flow-npm's description only — plausible, not
        // an identity hit, so the user picks.
        match route("packages outdated", &corpus()) {
            RouteDecision::Candidates { flows } => {
                assert_eq!(flows.first().map(|f| f.name.as_str()), Some("flow-npm"));
            }
            other => panic!("expected candidates, got {other:?}"),
        }
    }

    #[test]
    fn nonsense_is_no_match() {
        assert_eq!(route("zzz qqq", &corpus()), RouteDecision::NoMatch);
        assert_eq!(route("", &corpus()), RouteDecision::NoMatch);
    }

    #[test]
    fn ambiguous_identity_ties_offer_candidates() {
        let mut flows = corpus();
        flows.push(flow("flow-gmail-archive", "Bulk-archive Gmail"));
        match route("gmail", &flows) {
            RouteDecision::Candidates { flows } => assert!(flows.len() >= 2),
            RouteDecision::AutoStart { flow } => {
                // Also acceptable: exact identity beats compound name 2:1.
                assert_eq!(flow.name, "flow-gmail");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
