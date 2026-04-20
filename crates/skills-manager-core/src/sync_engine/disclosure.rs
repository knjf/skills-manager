use crate::skill_store::{DisclosureMode, PackRecord, SkillRecord};
use std::path::{Path, PathBuf};

/// A pack paired with its resolved skills — input to the desired-state resolver.
pub struct PackWithSkills<'a> {
    pub pack: &'a PackRecord,
    pub skills: &'a [SkillRecord],
}

/// A single desired entry in an agent's skills directory.
pub struct DesiredEntry {
    pub target_path: PathBuf,
    pub kind: EntryKind,
}

/// What kind of entry should be materialized at a given target path.
pub enum EntryKind {
    Skill { skill_name: String },
    Router { pack_name: String },
}

/// Compute the desired set of entries for an agent's skills directory under
/// the given disclosure mode.
///
/// - `Full`: materialize every skill from every pack; no routers.
/// - `Hybrid`: materialize skills from essential packs; emit router entries
///   for non-essential packs.
/// - `RouterOnly`: emit router entries only for non-essential packs.
///   (Essential packs contribute nothing in this mode.)
pub fn resolve_desired_state(
    agent_skills_dir: &Path,
    packs: &[PackWithSkills<'_>],
    mode: DisclosureMode,
    excluded_skills: &std::collections::HashSet<String>,
) -> Vec<DesiredEntry> {
    let mut out = Vec::new();
    for p in packs {
        let materialize = match mode {
            DisclosureMode::Full => true,
            DisclosureMode::Hybrid => p.pack.is_essential,
            DisclosureMode::RouterOnly => false,
        };
        if materialize {
            for s in p.skills {
                if excluded_skills.contains(&s.name) {
                    continue;
                }
                out.push(DesiredEntry {
                    target_path: agent_skills_dir.join(&s.name),
                    kind: EntryKind::Skill {
                        skill_name: s.name.clone(),
                    },
                });
            }
        }
        if mode != DisclosureMode::Full && !p.pack.is_essential {
            out.push(DesiredEntry {
                target_path: agent_skills_dir.join(format!("pack-{}", p.pack.name)),
                kind: EntryKind::Router {
                    pack_name: p.pack.name.clone(),
                },
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(name: &str, essential: bool) -> PackRecord {
        PackRecord {
            id: format!("p-{name}"),
            name: name.into(),
            description: None,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
            router_description: None,
            router_body: None,
            is_essential: essential,
            router_updated_at: None,
            router_when_to_use: None,
        }
    }

    fn skill(name: &str) -> SkillRecord {
        SkillRecord {
            id: format!("s-{name}"),
            name: name.into(),
            description: None,
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
        }
    }

    #[test]
    fn full_mode_materializes_everything_no_routers() {
        let ess = pack("essential", true);
        let dom = pack("dev-fe", false);
        let ess_skills = vec![skill("find-skills")];
        let dom_skills = vec![skill("frontend-design")];
        let packs = vec![
            PackWithSkills {
                pack: &ess,
                skills: &ess_skills,
            },
            PackWithSkills {
                pack: &dom,
                skills: &dom_skills,
            },
        ];
        let out = resolve_desired_state(
            Path::new("/cc"),
            &packs,
            DisclosureMode::Full,
            &std::collections::HashSet::new(),
        );
        let paths: Vec<_> = out.iter().map(|e| e.target_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/cc/find-skills")));
        assert!(paths.contains(&PathBuf::from("/cc/frontend-design")));
        assert!(!paths.iter().any(|p| p.to_string_lossy().contains("pack-")));
    }

    #[test]
    fn hybrid_mode_keeps_essential_skills_and_emits_routers_for_domain() {
        let ess = pack("essential", true);
        let dom = pack("dev-fe", false);
        let ess_skills = vec![skill("find-skills")];
        let dom_skills = vec![skill("frontend-design")];
        let packs = vec![
            PackWithSkills {
                pack: &ess,
                skills: &ess_skills,
            },
            PackWithSkills {
                pack: &dom,
                skills: &dom_skills,
            },
        ];
        let out = resolve_desired_state(
            Path::new("/cc"),
            &packs,
            DisclosureMode::Hybrid,
            &std::collections::HashSet::new(),
        );
        let paths: Vec<_> = out.iter().map(|e| e.target_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/cc/find-skills")));
        assert!(!paths.contains(&PathBuf::from("/cc/frontend-design")));
        assert!(paths.contains(&PathBuf::from("/cc/pack-dev-fe")));
        assert!(!paths.iter().any(|p| p.ends_with("pack-essential")));
    }

    #[test]
    fn router_only_emits_only_routers_for_non_essential() {
        let ess = pack("essential", true);
        let dom = pack("mkt-seo", false);
        let ess_skills = vec![skill("find-skills")];
        let dom_skills = vec![skill("seo-audit")];
        let packs = vec![
            PackWithSkills {
                pack: &ess,
                skills: &ess_skills,
            },
            PackWithSkills {
                pack: &dom,
                skills: &dom_skills,
            },
        ];
        let out = resolve_desired_state(
            Path::new("/cc"),
            &packs,
            DisclosureMode::RouterOnly,
            &std::collections::HashSet::new(),
        );
        let paths: Vec<_> = out.iter().map(|e| e.target_path.clone()).collect();
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&PathBuf::from("/cc/pack-mkt-seo")));
    }

    #[test]
    fn excluded_skills_filtered_in_full_mode() {
        use std::collections::HashSet;
        let p = pack("p1", false);
        let skills = vec![skill("alpha"), skill("beta")];
        let packs = vec![PackWithSkills {
            pack: &p,
            skills: &skills,
        }];
        let mut excluded = HashSet::new();
        excluded.insert("beta".to_string());

        let entries = resolve_desired_state(
            std::path::Path::new("/cc"),
            &packs,
            DisclosureMode::Full,
            &excluded,
        );

        let names: Vec<_> = entries
            .iter()
            .map(|e| e.target_path.to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"/cc/alpha".to_string()));
        assert!(!names.contains(&"/cc/beta".to_string()));
    }

    #[test]
    fn excluded_skills_does_not_affect_routers_in_hybrid() {
        use std::collections::HashSet;
        let p = pack("mkt", false);
        let skills = vec![skill("alpha"), skill("beta")];
        let packs = vec![PackWithSkills {
            pack: &p,
            skills: &skills,
        }];
        let mut excluded = HashSet::new();
        excluded.insert("alpha".to_string());
        excluded.insert("beta".to_string());

        let entries = resolve_desired_state(
            std::path::Path::new("/cc"),
            &packs,
            DisclosureMode::Hybrid,
            &excluded,
        );

        // Pack is non-essential and we're in hybrid: skills are vault-only,
        // so excluded set is irrelevant for skills here. But the router MUST still be emitted.
        let paths: Vec<_> = entries
            .iter()
            .map(|e| e.target_path.to_string_lossy().to_string())
            .collect();
        assert_eq!(paths, vec!["/cc/pack-mkt".to_string()]);
    }
}
