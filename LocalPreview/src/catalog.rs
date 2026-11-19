use std::collections::{BTreeMap, BTreeSet};

/// Strict numeric subset of the user's catalogue; no YAML code/tags are evaluated.
pub fn parse(text: &str) -> Result<String, String> {
    let allowed = ["character", "skin", "title", "item", "loading_image", "emoji", "endings"];
    let mut sections: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut current = String::new();
    for (index, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let raw = raw.split('#').next().unwrap_or("").trim_end();
        let line = raw.trim();
        if line.is_empty() { continue; }
        let fail = || format!("catalogue line {} is invalid", index + 1);
        if !raw.starts_with(char::is_whitespace) && line.ends_with(':') {
            let section = line.trim_end_matches(':');
            if !allowed.contains(&section) { return Err(fail()); }
            if sections.contains_key(section) { return Err(fail()); }
            current = section.into();
            sections.insert(current.clone(), BTreeSet::new());
        } else if current == "emoji" {
            // Emoji is accepted for compatibility, but is outside the preview scope.
            let value = line.strip_prefix("- ").unwrap_or(line).trim_end_matches(':');
            if value.parse::<u32>().is_err() { return Err(fail()); }
        } else if let Some(value) = line.strip_prefix("- ") {
            let id = value.trim().parse::<u32>().map_err(|_| fail())?;
            if id == 0 || current.is_empty() { return Err(fail()); }
            sections.get_mut(&current).ok_or_else(fail)?.insert(id);
        } else { return Err(fail()); }
    }
    if sections.get("character").is_none_or(|v| v.is_empty()) ||
        sections.get("skin").is_none_or(|v| v.is_empty()) {
        return Err("catalogue needs non-empty character and skin lists".into());
    }
    let mut out = String::from("{\n");
    for (key, ids) in sections {
        out.push_str(&format!("{key}={{"));
        for id in ids { out.push_str(&format!("{id},")); }
        out.push_str("},\n");
    }
    out.push('}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn numeric_only_and_indented_lists() {
        assert!(parse("character:\n  - 200001\nskin:\n - 400101 # skin").is_ok());
        assert!(parse("character:\n- 200001\nskin:\n- 400101\nevil=os.execute('x'):").is_err());
        assert!(parse("character:\n- 200001\nskin:\n- nope").is_err());
    }
}

