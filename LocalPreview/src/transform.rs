#[derive(Clone, Debug)]
pub struct Rule { pub key: String, pub function: String, pub code: String }

pub fn rules(text: &str) -> Result<Vec<Rule>, String> {
    let mut result = Vec::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<_> = line.splitn(3, '\t').collect();
        if parts.len() != 3 { return Err(format!("adapter line {} needs three tab-separated columns", index+1)); }
        result.push(Rule { key: parts[0].into(), function: parts[1].into(), code: parts[2].into() });
    }
    if !result.iter().any(|r| r.key == "network") { return Err("network guard rule missing".into()); }
    Ok(result)
}

#[derive(Debug)]
struct Token<'a> { text: &'a str, start: usize, end: usize }

fn long_end(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&b'[') { return None; }
    let mut p = at+1;
    while bytes.get(p) == Some(&b'=') { p+=1; }
    if bytes.get(p) != Some(&b'[') { return None; }
    let equals = p-at-1;
    p+=1;
    while p < bytes.len() {
        if bytes[p] == b']' && bytes.get(p+1..p+1+equals).is_some_and(|s| s.iter().all(|c| *c==b'='))
            && bytes.get(p+1+equals) == Some(&b']') { return Some(p+equals+2); }
        p+=1;
    }
    Some(bytes.len())
}

fn lex(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes(); let mut p=0; let mut out=Vec::new();
    while p<bytes.len() {
        if bytes[p].is_ascii_whitespace() { p+=1; continue; }
        if bytes.get(p..p+2)==Some(b"--") {
            if let Some(end)=long_end(bytes,p+2) { p=end; }
            else { while p<bytes.len() && bytes[p]!=b'\n' { p+=1; } }
            continue;
        }
        if bytes[p]==b'\'' || bytes[p]==b'"' {
            let quote=bytes[p]; p+=1;
            while p<bytes.len() { if bytes[p]==b'\\' { p=(p+2).min(bytes.len()); }
                else if bytes[p]==quote { p+=1; break; } else { p+=1; } }
            continue;
        }
        if let Some(end)=long_end(bytes,p) { p=end; continue; }
        let start=p;
        if bytes[p].is_ascii_alphanumeric() || bytes[p]==b'_' {
            p+=1; while p<bytes.len() && (bytes[p].is_ascii_alphanumeric() || bytes[p]==b'_') { p+=1; }
        } else {
            // UTF-8 outside a string is skipped as one scalar, never sliced midway.
            p+=source[p..].chars().next().unwrap().len_utf8();
        }
        out.push(Token { text:&source[start..p], start, end:p });
    }
    out
}

pub struct Output { pub source: String, pub matched: Vec<String> }

/// Extract one complete function from a minified dump for execution tests.
pub fn function_source<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let tokens=lex(source);let qualified=lex(name);
    for (i,t) in tokens.iter().enumerate() {
        if t.text!="function" || i+qualified.len()+1>=tokens.len() {continue;}
        if !qualified.iter().enumerate().all(|(j,q)|tokens[i+j+1].text==q.text) {continue;}
        if tokens[i+qualified.len()+1].text!="(" {continue;}
        let mut depth=0;
        for token in &tokens[i..] {
            match token.text {
                "function"|"if"|"do"|"repeat"=>depth+=1,
                "end"|"until"=>depth-=1,
                _=>{}
            }
            if depth==0 {return Some(&source[t.start..token.end]);}
        }
    }
    None
}

/// Insert into function bodies themselves; captured delegates keep these instructions.
/// Strings/comments are excluded and obfuscated argument names come from the live header.
pub fn rewrite(source: &str, adapter: &[Rule], prelude: &str) -> Result<Output, String> {
    let source=source.trim_start_matches('\u{feff}');
    if source.starts_with("-- MJLocalTrialNative") { return Ok(Output {source:source.into(),matched:vec![]}); }
    let tokens=lex(source); let mut edits=Vec::new(); let mut matched=Vec::new();
    let mut network=false;
    for rule in adapter {
        let qualified=lex(&rule.function);
        let mut found=Vec::new();
        for i in 0..tokens.len() {
            if tokens[i].text!="function" { continue; }
            let from=i+1; let open=from+qualified.len();
            if open>=tokens.len() || tokens[open].text!="(" { continue; }
            if !qualified.iter().enumerate().all(|(j,t)| tokens[from+j].text==t.text) { continue; }
            let Some(close)=(open+1..tokens.len()).find(|j|tokens[*j].text==")") else { continue; };
            let params: Vec<_>=tokens[open+1..close].iter().filter(|t|t.text!=",").map(|t|t.text).collect();
            let mut code=rule.code.clone();
            for (j,arg) in params.iter().enumerate() { code=code.replace(&format!("{{{{{}}}}}",j+1),arg); }
            if code.contains("{{") { return Err(format!("{}: incompatible arguments",rule.key)); }
            found.push((tokens[close].end,format!("\n{code}\n")));
        }
        if found.len()>1 { return Err(format!("{}: ambiguous function definition",rule.key)); }
        if !found.is_empty() { network|=rule.key=="network"; matched.push(rule.key.clone()); edits.extend(found); }
    }
    if matched.is_empty() { return Ok(Output{source:source.into(),matched}); }
    if network {
        let mut depth=0i32; let mut tail=source.len();
        for token in &tokens {
            match token.text {
                "function"|"if"|"do"|"repeat"=>depth+=1,
                "end"|"until"=>depth-=1,
                "return" if depth==0 => { tail=token.start; break; }
                _=>{}
            }
        }
        edits.push((tail,"\nMJLocalTrial.guard_ready=true\n".into()));
    }
    edits.sort_by_key(|e|e.0);
    let mut out=String::from("-- MJLocalTrialNative\n");out.push_str(prelude);out.push('\n');
    let mut at=0;
    for (offset,code) in edits { out.push_str(&source[at..offset]);out.push_str(&code);at=offset; }
    out.push_str(&source[at..]);
    Ok(Output{source:out,matched})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ignores_strings_and_binds_live_arguments() {
        let rules=vec![Rule{key:"network".into(),function:"Net.Send".into(),code:"if handle({{1}}) then return end;".into()}];
        let source="local s='function Net.Send(x)' -- function Net.Send(y)\nfunction Net.Send(q) return q end;return Net";
        let r=rewrite(source,&rules,"").unwrap();
        assert_eq!(r.matched,vec!["network"]);assert!(r.source.contains("handle(q)"));
        assert!(r.source.contains("MJLocalTrial.guard_ready=true\nreturn Net"));
        assert_eq!(rewrite(&r.source,&rules,"").unwrap().matched.len(),0);
    }
    #[test] fn incompatible_signature_is_rejected() {
        let rules=vec![Rule{key:"network".into(),function:"Net.Send".into(),code:"use({{2}})".into()}];
        assert!(rewrite("function Net.Send(a)end",&rules,"").is_err());
    }
}
