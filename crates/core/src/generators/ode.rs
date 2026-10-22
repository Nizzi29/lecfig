//! `ode("m y'' + d y' + c y = F")`: a linear ODE with constant coefficients as a block
//! diagram (integrator chain with feedback).
//!
//! Derivatives are written with primes (`y''`) or dots (`dot(y)`, `dot.double(y)`); the
//! differentiated variable is the output, every other term is an input. Coefficients are
//! Typst math (`2 D omega_0`, `omega_0^2`, `1/T`). The equation is solved for the highest
//! derivative; each remaining term enters through its own summing junction.
//! Options: `blocks: "time" | "laplace"`, `normalize: "sum" | "gains"`, `out: "y"`.

use serde_json::{Map, Value};

use super::{err, opt_str};
use crate::model::*;

#[derive(Clone, Debug, PartialEq)]
struct Term {
    /// Sign after moving the term to the left-hand side.
    sign: i32,
    coef: String,
    signal: String,
    order: u32,
}

/// Splits one side of the equation into signed terms at top-level `+`/`-`.
fn split_terms(side: &str) -> Vec<(i32, String)> {
    let mut out = vec![];
    let mut depth = 0;
    let mut sign = 1;
    let mut cur = String::new();
    let mut prev = ' ';
    for c in side.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        // `^-1` and `_-` are exponents/subscripts, not term boundaries.
        let boundary = depth == 0 && (c == '+' || c == '-' || c == '−') && !matches!(prev, '^' | '_');
        if boundary {
            if !cur.trim().is_empty() {
                out.push((sign, cur.trim().to_string()));
            }
            cur.clear();
            sign = if c == '+' { 1 } else { -1 };
        } else {
            cur.push(c);
        }
        if !c.is_whitespace() {
            prev = c;
        }
    }
    if !cur.trim().is_empty() {
        out.push((sign, cur.trim().to_string()));
    }
    out
}

/// Splits a term into factors at top-level spaces; `3y` becomes `3`, `y`.
fn factors(term: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut depth = 0;
    let mut quoted = false;
    let mut cur = String::new();
    for c in term.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 && !quoted {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    // A leading number glued to a name: 3y → 3 y.
    if let Some(last) = out.last().cloned() {
        let digits: String = last.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if !digits.is_empty() && digits.len() < last.len() && last[digits.len()..].starts_with(|c: char| c.is_alphabetic()) {
            out.pop();
            out.push(digits.clone());
            out.push(last[digits.len()..].to_string());
        }
    }
    out
}

fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_alphabetic())
        && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '"')
}

/// A signal factor: `y`, `y''`, `dot(y)`, `dot.double(y)`, `dot.triple(y)`, `dot.quad(y)`.
/// Returns (name, order, written with dots).
fn signal(f: &str) -> Option<(String, u32, bool)> {
    for (prefix, order) in [("dot(", 1), ("dot.double(", 2), ("dot.triple(", 3), ("dot.quad(", 4)] {
        if let Some(inner) = f.strip_prefix(prefix).and_then(|r| r.strip_suffix(')')) {
            let inner = inner.trim();
            return is_name(inner).then(|| (inner.to_string(), order, true));
        }
    }
    let name = f.trim_end_matches('\'');
    let order = (f.len() - name.len()) as u32;
    is_name(name).then(|| (name.to_string(), order, false))
}

fn mentions(text: &str, name: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|w| w == name)
}

fn deriv(name: &str, k: u32, dots: bool) -> String {
    match (dots, k) {
        (_, 0) => name.to_string(),
        (true, 1) => format!("dot({name})"),
        (true, 2) => format!("dot.double({name})"),
        (true, 3) => format!("dot.triple({name})"),
        (true, 4) => format!("dot.quad({name})"),
        (true, k) => format!("{name}^(({k}))"),
        (false, k) => format!("{name}{}", "'".repeat(k as usize)),
    }
}

/// Wraps a coefficient in brackets when it is more than one factor.
fn group(c: &str) -> String {
    if c.chars().any(|ch| ch.is_whitespace() || matches!(ch, '+' | '-' | '/')) { format!("({c})") } else { c.to_string() }
}

struct Parsed {
    out: String,
    dots: bool,
    terms: Vec<Term>,
}

fn parse(expr: &str, out_hint: Option<&str>) -> Result<Parsed, Error> {
    let sides: Vec<&str> = expr.split('=').collect();
    if sides.len() != 2 {
        return err("ode: write one equation with one '=', e.g. \"m y'' + d y' + c y = F\"");
    }
    let mut raw = vec![];
    for (side, flip) in [(sides[0], 1), (sides[1], -1)] {
        for (sign, text) in split_terms(side) {
            raw.push((sign * flip, text));
        }
    }
    // The output is the variable with the highest derivative.
    let mut dots = false;
    let out = match out_hint {
        Some(o) => o.to_string(),
        None => {
            let mut best: Vec<(String, u32)> = vec![];
            for (_, text) in &raw {
                if let Some((name, order, d)) = factors(text).last().and_then(|f| signal(f)) {
                    if order == 0 {
                        continue;
                    }
                    dots |= d;
                    match best.first().map(|b| b.1) {
                        Some(top) if order < top => {}
                        Some(top) if order == top => {
                            if !best.iter().any(|b| b.0 == name) {
                                best.push((name, order));
                            }
                        }
                        _ => best = vec![(name, order)],
                    }
                }
            }
            match best.len() {
                0 => return err("ode: no derivative found; write derivatives as y' or dot(y)"),
                1 => best.pop().unwrap().0,
                _ => {
                    let names: Vec<_> = best.iter().map(|b| b.0.as_str()).collect();
                    return err(format!("ode: several variables have the highest derivative ({}); pick one with out: \"…\"", names.join(", ")));
                }
            }
        }
    };
    let mut terms = vec![];
    for (sign, text) in raw {
        let fs = factors(&text);
        let last = fs.last().cloned().unwrap_or_default();
        let coef = if fs.len() > 1 { fs[..fs.len() - 1].join(" ") } else { "1".to_string() };
        if mentions(&coef, &out) {
            return err(format!("ode: '{text}' is nonlinear in {out}; nonlinear terms are on the backlog"));
        }
        match signal(&last) {
            Some((name, order, d)) if name == out => {
                dots |= d;
                terms.push(Term { sign, coef, signal: name, order });
            }
            Some((name, 0, _)) => terms.push(Term { sign, coef, signal: name, order: 0 }),
            Some((name, _, _)) => {
                return err(format!(
                    "ode: '{text}' differentiates the input {name}; input derivatives need a canonical form (backlog)"
                ));
            }
            None if mentions(&last, &out) => {
                return err(format!("ode: '{text}' is nonlinear in {out}; nonlinear terms are on the backlog"));
            }
            // A constant or an expression without the output: treat it as an input signal.
            None => terms.push(Term { sign, coef, signal: last, order: 0 }),
        }
    }
    let outs: Vec<&Term> = terms.iter().filter(|t| t.signal == out).collect();
    for (i, a) in outs.iter().enumerate() {
        if outs[i + 1..].iter().any(|b| b.order == a.order) {
            return err(format!("ode: {} appears twice; combine the terms first", deriv(&out, a.order, dots)));
        }
    }
    Ok(Parsed { out, dots, terms })
}

pub fn generate(expr: &str, o: &Map<String, Value>) -> Result<Figure, Error> {
    let p = parse(expr, opt_str(o, "out"))?;
    let normalize_gains = opt_str(o, "normalize") == Some("gains");
    let blocks = opt_str(o, "blocks").unwrap_or("time");
    let lead = p.terms.iter().filter(|t| t.signal == p.out).max_by_key(|t| t.order).unwrap().clone();
    let n = lead.order;
    let an = lead.coef.clone();
    // Sign each remaining term gets at the sum: y^(n) = 1/a_n Σ ρ_i c_i X_i.
    let rho = |t: &Term| -t.sign * lead.sign;
    let mut feedback: Vec<&Term> = p.terms.iter().filter(|t| t.signal == p.out && t.order < n).collect();
    feedback.sort_by_key(|t| t.order);
    let inputs: Vec<&Term> = p.terms.iter().filter(|t| t.signal != p.out).collect();
    let gain_label = |c: &str| -> Option<String> {
        match (normalize_gains && an != "1", c) {
            (false, "1") => None,
            (false, c) => Some(c.to_string()),
            (true, "1") => Some(format!("1/{}", group(&an))),
            (true, c) => Some(format!("{}/{}", group(c), group(&an))),
        }
    };

    let mut b = Block { style: BlockStyle { blocks: blocks.into() }, ..Default::default() };
    let add = |b: &mut Block, id: String, kind: &str, at: (f64, f64), label: String| {
        b.nodes.push(BNode { id: id.clone(), kind: kind.into(), at: Pt(at.0, at.1), label, extra: Map::new() });
        id
    };
    let edge = |b: &mut Block, from: &str, to: &str, label: String, sign: &str, bends: Vec<Pt>| {
        b.edges.push(BEdge { from: from.into(), to: to.into(), label, sign: sign.into(), bends, extra: Map::new() });
    };
    let sign_str = |s: i32| if s > 0 { "+" } else { "-" };

    // Main line, left to right: (node id, sign of the main-line edge into it if a sum).
    let mut main: Vec<String> = vec![];
    let mut col = 0.0;
    let mut first_sign = "+";
    if let Some(i0) = inputs.first() {
        main.push(add(&mut b, "in1".into(), "io", (col, 0.0), i0.signal.clone()));
        col += 1.0;
        if let Some(g) = gain_label(&i0.coef) {
            main.push(add(&mut b, "kin1".into(), "gain", (col, 0.0), g));
            col += 1.0;
        }
        first_sign = sign_str(rho(i0));
    }
    for (j, inp) in inputs.iter().enumerate().skip(1) {
        let s = add(&mut b, format!("sin{}", j + 1), "sum", (col, 0.0), String::new());
        let io = format!("in{}", j + 1);
        match gain_label(&inp.coef) {
            Some(g) => {
                add(&mut b, io.clone(), "io", (col, -2.0), inp.signal.clone());
                let k = add(&mut b, format!("kin{}", j + 1), "gain", (col, -1.0), g);
                edge(&mut b, &io, &k, String::new(), "", vec![]);
                edge(&mut b, &k, &s, String::new(), sign_str(rho(inp)), vec![]);
            }
            None => {
                add(&mut b, io.clone(), "io", (col, -1.0), inp.signal.clone());
                edge(&mut b, &io, &s, String::new(), sign_str(rho(inp)), vec![]);
            }
        }
        main.push(s);
        col += 1.0;
    }
    let mut fb_sums = vec![];
    for t in &feedback {
        let s = add(&mut b, format!("s{}", t.order), "sum", (col, 0.0), String::new());
        fb_sums.push(s.clone());
        main.push(s);
        col += 1.0;
    }
    if !normalize_gains && an != "1" {
        main.push(add(&mut b, "kn".into(), "gain", (col, 0.0), format!("1/{}", group(&an))));
        col += 1.0;
    }
    // The edge leaving the sums / normalising gain carries y^(n).
    let top_index = main.len();
    let mut taps = std::collections::HashMap::new();
    let mut labels = std::collections::HashMap::new();
    labels.insert(top_index, deriv(&p.out, n, p.dots));
    for m in 1..=n {
        let k = n - m; // this integrator outputs y^(k)
        main.push(add(&mut b, format!("i{m}"), "integrator", (col, 0.0), String::new()));
        col += 1.0;
        if k > 0 {
            labels.insert(main.len(), deriv(&p.out, k, p.dots));
        }
        if feedback.iter().any(|t| t.order == k) {
            let br = add(&mut b, format!("b{k}"), "branch", (col, 0.0), String::new());
            taps.insert(k, (br.clone(), col));
            main.push(br);
            col += 1.0;
        }
    }
    main.push(add(&mut b, "out".into(), "io", (col, 0.0), p.out.clone()));

    // Chain the main line; edges into sums carry their sign.
    let fb_count = fb_sums.len();
    let mut first_sum_seen = false;
    for w in 0..main.len() - 1 {
        let (from, to) = (&main[w], &main[w + 1]);
        let to_sum = b.nodes.iter().any(|nd| &nd.id == to && nd.kind == "sum");
        let sign = if to_sum {
            let s = if !first_sum_seen && !inputs.is_empty() { first_sign } else { "+" };
            first_sum_seen = true;
            s
        } else {
            ""
        };
        let label = labels.get(&(w + 1)).cloned().unwrap_or_default();
        edge(&mut b, from, to, label, sign, vec![]);
    }

    // Feedback loops: innermost (highest order) on the first row below the main line.
    for (idx, t) in feedback.iter().enumerate() {
        let row = (fb_count - idx) as f64;
        let (br, tap_col) = taps[&t.order].clone();
        let sum = &fb_sums[idx];
        let sum_col = b.nodes.iter().find(|nd| &nd.id == sum).unwrap().at.0;
        let sign = sign_str(rho(t));
        match gain_label(&t.coef) {
            Some(g) => {
                let k = add(&mut b, format!("k{}", t.order), "gain", (tap_col, row), g);
                edge(&mut b, &br, &k, String::new(), "", vec![]);
                edge(&mut b, &k, sum, String::new(), sign, vec![Pt(sum_col, row)]);
            }
            None => edge(&mut b, &br, sum, String::new(), sign, vec![Pt(tap_col, row), Pt(sum_col, row)]),
        }
    }

    // The rearranged equation, for `show-steps`.
    let mut rhs = String::new();
    for t in inputs.iter().chain(feedback.iter().rev()) {
        let x = if t.signal == p.out { deriv(&p.out, t.order, p.dots) } else { t.signal.clone() };
        let factor = if t.coef == "1" { x } else { format!("{} {x}", group(&t.coef)) };
        let s = rho(t);
        if rhs.is_empty() {
            rhs = if s > 0 { factor } else { format!("-{factor}") };
        } else {
            rhs += &format!(" {} {factor}", if s > 0 { "+" } else { "-" });
        }
    }
    if rhs.is_empty() {
        rhs = "0".into();
    }
    let steps = if an == "1" {
        format!("{} = {rhs}", deriv(&p.out, n, p.dots))
    } else {
        format!("{} = 1/{} ({rhs})", deriv(&p.out, n, p.dots), group(&an))
    };
    let mut opts = Map::new();
    opts.insert("steps".into(), Value::String(steps));
    b.source = Some(Source { generator: "ode".into(), expr: expr.into(), options: opts });
    Ok(Figure::Block(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(expr: &str) -> Block {
        match generate(expr, &Map::new()).unwrap() {
            Figure::Block(b) => b,
            _ => unreachable!(),
        }
    }

    fn label(b: &Block, id: &str) -> String {
        b.nodes.iter().find(|n| n.id == id).unwrap().label.clone()
    }

    #[test]
    fn mass_spring_damper() {
        let b = block("m y'' + d y' + c y = F");
        assert_eq!(label(&b, "in1"), "F");
        assert_eq!(label(&b, "kn"), "1/m");
        assert_eq!(label(&b, "k1"), "d");
        assert_eq!(label(&b, "k0"), "c");
        let signs: Vec<_> = b.edges.iter().filter(|e| !e.sign.is_empty()).map(|e| (e.to.as_str(), e.sign.as_str())).collect();
        assert!(signs.contains(&("s0", "+")));
        assert!(signs.contains(&("s0", "-")));
        assert!(signs.contains(&("s1", "-")));
        assert_eq!(b.source.as_ref().unwrap().options["steps"], "y'' = 1/m (F - d y' - c y)");
    }

    #[test]
    fn dots_and_symbolic_coefficients() {
        let b = block("dot.double(x) + 2 D omega_0 dot(x) + omega_0^2 x = omega_0^2 u");
        assert_eq!(label(&b, "k1"), "2 D omega_0");
        assert_eq!(label(&b, "kin1"), "omega_0^2");
        assert!(b.edges.iter().any(|e| e.label == "dot.double(x)"));
        assert!(!b.nodes.iter().any(|n| n.id == "kn"), "leading coefficient 1 needs no gain");
    }

    #[test]
    fn signs_follow_the_equation() {
        let b = block("y'' - 3y' = u");
        let fb = b.edges.iter().find(|e| e.to == "s1" && e.from == "k1").unwrap();
        assert_eq!(fb.sign, "+");
    }

    #[test]
    fn fourth_order_chain() {
        let b = block("y'''' + a_3 y''' + a_2 y'' + a_1 y' + a_0 y = u");
        assert_eq!(b.nodes.iter().filter(|n| n.kind == "integrator").count(), 4);
        assert_eq!(b.nodes.iter().filter(|n| n.kind == "sum").count(), 4);
    }

    #[test]
    fn normalize_into_gains() {
        let mut o = Map::new();
        o.insert("normalize".into(), "gains".into());
        let Figure::Block(b) = generate("m y'' + d y' + c y = F", &o).unwrap() else { panic!() };
        assert_eq!(label(&b, "k1"), "d/m");
        assert_eq!(label(&b, "kin1"), "1/m");
    }

    #[test]
    fn clear_errors_for_backlog_cases() {
        let e = |s: &str| generate(s, &Map::new()).unwrap_err().0;
        assert!(e("y'' + sin(y) = u").contains("nonlinear"));
        assert!(e("y'' + y y' = u").contains("nonlinear"));
        assert!(e("y'' + y = u'").contains("input derivatives"));
        assert!(e("y + 2 = u").contains("no derivative"));
    }
}
