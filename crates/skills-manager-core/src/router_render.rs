use crate::skill_store::{PackRecord, SkillRecord};
use std::path::Path;

/// Render the SKILL.md content for a pack router.
///
/// If `pack.router_body` is set, use it as-is. Otherwise auto-render a skill
/// table from the pack's skills. The frontmatter uses `router_description`
/// (falling back to a placeholder).
pub fn render_router_skill_md(
    pack: &PackRecord,
    skills: &[SkillRecord],
    vault_root: &Path,
) -> String {
    let desc = pack
        .router_description
        .as_deref()
        .unwrap_or("Router for pack — description pending generation.");
    let body = pack
        .router_body
        .clone()
        .unwrap_or_else(|| auto_render_body(pack, skills, vault_root));

    let mut frontmatter = format!(
        "---\nname: pack-{}\ndescription: {}\n",
        pack.name,
        escape_yaml_scalar(desc),
    );
    if let Some(when) = pack
        .router_when_to_use
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        frontmatter.push_str(&format!("when_to_use: {}\n", escape_yaml_scalar(when)));
    }
    frontmatter.push_str("---\n\n");
    format!("{}{}\n", frontmatter, body)
}

fn auto_render_body(pack: &PackRecord, skills: &[SkillRecord], vault_root: &Path) -> String {
    let mut out = format!(
        "# Pack: {}\n\n\
        揀一個 skill，用 `Read` tool 讀對應 SKILL.md，跟住做。\n\n\
        | Skill | 用途 | 路徑 |\n|---|---|---|\n",
        pack.name,
    );
    for s in skills {
        let summary = skill_row_summary(s);
        out.push_str(&format!(
            "| `{}` | {} | `{}/{}/SKILL.md` |\n",
            s.name,
            summary,
            vault_root.display(),
            s.name,
        ));
    }
    out
}

/// Pick the per-row summary: prefer `description_router` when set and non-empty,
/// else fall back to first sentence of `description`, else empty.
fn skill_row_summary(s: &SkillRecord) -> String {
    if let Some(r) = s
        .description_router
        .as_deref()
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        return r.to_string();
    }
    s.description
        .as_deref()
        .unwrap_or("")
        .split_terminator(['.', '。'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn escape_yaml_scalar(s: &str) -> String {
    if s.contains('\n')
        || s.contains(':')
        || s.starts_with([
            '-', '?', '[', '{', '|', '>', '!', '@', '`', '#', '&', '*', '\'', '%', '"',
        ])
    {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pack(name: &str, router_desc: Option<&str>) -> PackRecord {
        PackRecord {
            id: format!("p-{name}"),
            name: name.into(),
            description: None,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
            router_description: router_desc.map(str::to_string),
            router_body: None,
            is_essential: false,
            router_updated_at: None,
            router_when_to_use: None,
        }
    }

    fn skill(name: &str, desc: &str) -> SkillRecord {
        SkillRecord {
            id: format!("s-{name}"),
            name: name.into(),
            description: Some(desc.into()),
            source_type: "local".into(),
            source_ref: None,
            source_ref_resolved: None,
            source_subpath: None,
            source_branch: None,
            source_revision: None,
            remote_revision: None,
            central_path: format!("/vault/{name}"),
            content_hash: None,
            enabled: true,
            created_at: 0,
            updated_at: 0,
            status: "active".into(),
            update_status: "idle".into(),
            last_checked_at: None,
            last_check_error: None,
            description_router: None,
        }
    }

    #[test]
    fn renders_frontmatter_with_pack_name() {
        let p = pack("mkt-seo", Some("When user mentions SEO"));
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/vault"));
        assert!(out.starts_with("---\nname: pack-mkt-seo"));
        assert!(out.contains("description: When user mentions SEO"));
    }

    #[test]
    fn auto_renders_skill_table_when_body_empty() {
        let p = pack("mkt-seo", Some("desc"));
        let skills = vec![
            skill("seo-audit", "Diagnose SEO issues. Use when..."),
            skill("ai-seo", "Optimize for LLM citations."),
        ];
        let out = render_router_skill_md(&p, &skills, &PathBuf::from("/vault"));
        assert!(out.contains("| `seo-audit` | Diagnose SEO issues | `/vault/seo-audit/SKILL.md` |"));
        assert!(
            out.contains("| `ai-seo` | Optimize for LLM citations | `/vault/ai-seo/SKILL.md` |")
        );
    }

    #[test]
    fn custom_router_body_is_used_as_is() {
        let mut p = pack("custom", Some("desc"));
        p.router_body = Some("# Custom body\n\nhand-written".into());
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/vault"));
        assert!(out.contains("# Custom body"));
        assert!(!out.contains("揀一個 skill"));
    }

    #[test]
    fn null_description_emits_placeholder() {
        let p = pack("x", None);
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(out.contains("description: Router for pack — description pending generation."));
    }

    #[test]
    fn yaml_special_chars_are_quoted() {
        let p = pack("x", Some("Trigger: SEO audit"));
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(out.contains("description: \"Trigger: SEO audit\""));
    }

    #[test]
    fn deterministic_for_same_input() {
        let p = pack("x", Some("d"));
        let skills = vec![skill("a", "x."), skill("b", "y.")];
        let a = render_router_skill_md(&p, &skills, &PathBuf::from("/v"));
        let b = render_router_skill_md(&p, &skills, &PathBuf::from("/v"));
        assert_eq!(a, b);
    }

    #[test]
    fn frontmatter_emits_when_to_use_when_set() {
        let mut p = pack("mkt", Some("Marketing domain"));
        p.router_when_to_use = Some("Use when user mentions SEO, CRO, PRD".into());
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(
            out.contains("when_to_use: Use when user mentions SEO, CRO, PRD"),
            "when_to_use missing; got:\n{out}"
        );
    }

    #[test]
    fn frontmatter_omits_when_to_use_when_none() {
        let p = pack("mkt", Some("Marketing domain"));
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(
            !out.contains("when_to_use:"),
            "when_to_use should not appear; got:\n{out}"
        );
    }

    #[test]
    fn table_uses_description_router_when_set() {
        let p = pack("mkt", Some("desc"));
        let mut s = skill(
            "seo-audit",
            "Full long original description with lots of words.",
        );
        s.description_router = Some("Short router line".into());
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(
            out.contains("| `seo-audit` | Short router line | `/v/seo-audit/SKILL.md` |"),
            "expected short router line; got:\n{out}"
        );
    }

    #[test]
    fn table_falls_back_to_description_when_router_desc_none() {
        let p = pack("mkt", Some("desc"));
        let s = skill("seo-audit", "First sentence. Second sentence.");
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(
            out.contains("| `seo-audit` | First sentence | `/v/seo-audit/SKILL.md` |"),
            "expected fallback to first-sentence truncation; got:\n{out}"
        );
    }

    #[test]
    fn table_falls_back_when_router_desc_is_empty_string() {
        let p = pack("mkt", Some("desc"));
        let mut s = skill("x", "Original desc.");
        s.description_router = Some("   ".into()); // whitespace-only
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(out.contains("| `x` | Original desc"));
    }

    #[test]
    fn yaml_escapes_quotes_in_when_to_use() {
        let mut p = pack("x", Some("d"));
        p.router_when_to_use = Some("Use when: \"quoted trigger\"".into());
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(
            out.contains("when_to_use: \"Use when: \\\"quoted trigger\\\"\""),
            "expected escaped quotes; got:\n{out}"
        );
    }
}
