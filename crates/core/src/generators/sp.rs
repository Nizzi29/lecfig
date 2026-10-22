//! `sp("R_1 + (R_2 || C_1)")`: series/parallel two-terminal networks.
//!
//! `+` is series, `||` parallel (binds tighter), parentheses group. An operand is any
//! Typst math text; its first letter picks the part: R/G resistor, C capacitor,
//! L inductor, U/V/E voltage source, I/J current source, D diode, S switch, anything
//! else an impedance box (so `j omega L` works).
//! Options: `terminals: false` drops the a/b terminals, `loop: true` closes the chain
//! with a return wire (for series circuits with a source).

use std::collections::HashMap;

use serde_json::{Map, Value};

use super::{err, opt_bool};
use crate::SCHEMA_VERSION;
use crate::model::*;

#[derive(Debug, PartialEq)]
enum Net {
    Leaf(String),
    Series(Vec<Net>),
    Parallel(Vec<Net>),
}

#[derive(Debug, PartialEq)]
enum Tok {
    Plus,
    Par,
    Open,
    Close,
    Ident(String),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, Error> {
    let c: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            ' ' | '\t' => i += 1,
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '|' if c.get(i + 1) == Some(&'|') => {
                out.push(Tok::Par);
                i += 2;
            }
            '∥' => {
                out.push(Tok::Par);
                i += 1;
            }
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            '|' => return err("sp: write '||' for parallel"),
            _ => {
                // An operand runs to the next operator or bracket; brackets right after
                // `_` or `^` and quoted text belong to it (R_(1 2), C_"load").
                let mut id = String::new();
                while i < c.len() && !matches!(c[i], '+' | '|' | '∥' | '(' | ')') {
                    if matches!(c[i], '_' | '^') && c.get(i + 1) == Some(&'(') {
                        let mut depth = 0;
                        id.push(c[i]);
                        i += 1;
                        while i < c.len() {
                            id.push(c[i]);
                            depth += match c[i] {
                                '(' => 1,
                                ')' => -1,
                                _ => 0,
                            };
                            i += 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    } else if c[i] == '"' {
                        id.push('"');
                        i += 1;
                        while i < c.len() && c[i] != '"' {
                            id.push(c[i]);
                            i += 1;
                        }
                        if i < c.len() {
                            id.push('"');
                            i += 1;
                        }
                    } else {
                        id.push(c[i]);
                        i += 1;
                    }
                }
                out.push(Tok::Ident(id.trim().to_string()));
            }
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn expr(&mut self) -> Result<Net, Error> {
        let mut items = vec![self.term()?];
        while self.peek() == Some(&Tok::Plus) {
            self.pos += 1;
            items.push(self.term()?);
        }
        Ok(flatten(items, true))
    }

    fn term(&mut self) -> Result<Net, Error> {
        let mut items = vec![self.factor()?];
        while self.peek() == Some(&Tok::Par) {
            self.pos += 1;
            items.push(self.factor()?);
        }
        Ok(flatten(items, false))
    }

    fn factor(&mut self) -> Result<Net, Error> {
        match self.toks.get(self.pos) {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(Net::Leaf(s))
            }
            Some(Tok::Open) => {
                self.pos += 1;
                let e = self.expr()?;
                if self.peek() != Some(&Tok::Close) {
                    return err("sp: missing ')'");
                }
                self.pos += 1;
                Ok(e)
            }
            Some(t) => err(format!("sp: unexpected {t:?}, expected a part or '('")),
            None => err("sp: expression ends too early"),
        }
    }
}

/// Merges nested series into series and parallel into parallel.
fn flatten(items: Vec<Net>, series: bool) -> Net {
    if items.len() == 1 {
        return items.into_iter().next().unwrap();
    }
    let mut out = vec![];
    for it in items {
        match it {
            Net::Series(v) if series => out.extend(v),
            Net::Parallel(v) if !series => out.extend(v),
            other => out.push(other),
        }
    }
    if series { Net::Series(out) } else { Net::Parallel(out) }
}

fn parse(expr: &str) -> Result<Net, Error> {
    let mut p = Parser { toks: tokenize(expr)?, pos: 0 };
    let net = p.expr()?;
    if p.pos != p.toks.len() {
        return err(format!("sp: unexpected {:?}", p.toks[p.pos]));
    }
    Ok(net)
}

fn part_for(label: &str) -> &'static str {
    match label.chars().find(|c| c.is_alphabetic()) {
        Some('R' | 'G') => "resistor",
        Some('C') => "capacitor",
        Some('L') => "inductor",
        Some('U' | 'V' | 'E') => "vsource",
        Some('I' | 'J') => "isource",
        Some('D') => "diode",
        Some('S') => "switch",
        _ => "impedance",
    }
}

const LEAF: f64 = 2.0;
const STUB: f64 = 1.0;
const GAP: f64 = 1.5;

struct Layout {
    elements: Vec<Element>,
    wires: Vec<Wire>,
    counts: HashMap<String, usize>,
}

impl Layout {
    fn wire(&mut self, pts: &[Pt]) {
        self.wires.push(Wire { points: pts.iter().map(|p| WirePt::At(*p)).collect(), ..Default::default() });
    }

    fn node(&mut self, at: Pt) {
        let id = self.next_id("n");
        self.elements.push(Element { id, kind: "node".into(), at: Some(at), ..Default::default() });
    }

    fn next_id(&mut self, prefix: &str) -> String {
        let n = self.counts.entry(prefix.to_string()).or_insert(0);
        *n += 1;
        format!("{prefix}{n}")
    }

    fn width(net: &Net) -> f64 {
        match net {
            Net::Leaf(_) => LEAF,
            Net::Series(v) => v.iter().map(Self::width).sum(),
            Net::Parallel(v) => v.iter().map(Self::width).fold(0.0, f64::max) + 2.0 * STUB,
        }
    }

    /// Places `net` with its left terminal at `at`; returns how far it reaches below `at`.
    fn place(&mut self, net: &Net, at: Pt) -> f64 {
        match net {
            Net::Leaf(label) => {
                let kind = part_for(label);
                let prefix = crate::parts::part(kind).map(|p| p.prefix).unwrap_or("Z").to_lowercase();
                let id = self.next_id(&prefix);
                self.elements.push(Element {
                    id,
                    kind: kind.into(),
                    from: Some(at),
                    to: Some(at.add(Pt(LEAF, 0.0))),
                    label: label.clone(),
                    ..Default::default()
                });
                0.0
            }
            Net::Series(items) => {
                let mut x = at.0;
                let mut depth: f64 = 0.0;
                for it in items {
                    depth = depth.max(self.place(it, Pt(x, at.1)));
                    x += Self::width(it);
                }
                depth
            }
            Net::Parallel(items) => {
                let inner = Self::width(net) - 2.0 * STUB;
                let (left, right) = (at.0 + STUB, at.0 + STUB + inner);
                let mut y = at.1;
                let mut rows = vec![];
                let mut depth = 0.0;
                for (k, it) in items.iter().enumerate() {
                    let w = Self::width(it);
                    let pad = (inner - w) / 2.0;
                    let d = self.place(it, Pt(left + pad, y));
                    if pad > 0.0 {
                        self.wire(&[Pt(left, y), Pt(left + pad, y)]);
                        self.wire(&[Pt(left + pad + w, y), Pt(right, y)]);
                    }
                    rows.push(y);
                    depth = at.1 - y + d;
                    if k + 1 < items.len() {
                        y -= d + GAP;
                    }
                }
                let last = *rows.last().unwrap();
                self.wire(&[at, Pt(left, at.1)]);
                self.wire(&[Pt(right, at.1), Pt(right + STUB, at.1)]);
                self.wire(&[Pt(left, at.1), Pt(left, last)]);
                self.wire(&[Pt(right, at.1), Pt(right, last)]);
                for &r in &rows[..rows.len() - 1] {
                    self.node(Pt(left, r));
                    self.node(Pt(right, r));
                }
                depth
            }
        }
    }
}

pub fn generate(expr: &str, o: &Map<String, Value>) -> Result<Figure, Error> {
    let net = parse(expr)?;
    let mut l = Layout { elements: vec![], wires: vec![], counts: HashMap::new() };
    let width = Layout::width(&net);
    let closed = opt_bool(o, "loop");
    let terminals = !closed && o.get("terminals").and_then(Value::as_bool).unwrap_or(true);
    let start = if terminals { Pt(1.0, 0.0) } else { Pt(0.0, 0.0) };
    let depth = l.place(&net, start);
    let end = start.add(Pt(width, 0.0));
    if terminals {
        l.elements.push(Element { id: "a".into(), kind: "terminal".into(), at: Some(Pt(0.0, 0.0)), label: "a".into(), ..Default::default() });
        l.elements.push(Element { id: "b".into(), kind: "terminal".into(), at: Some(end.add(Pt(1.0, 0.0))), label: "b".into(), ..Default::default() });
        l.wire(&[Pt(0.0, 0.0), start]);
        l.wire(&[end, end.add(Pt(1.0, 0.0))]);
    }
    if closed {
        let low = -(depth + GAP);
        l.wire(&[end, Pt(end.0, low), Pt(start.0, low), start]);
    }
    let flavor = if l.elements.iter().any(|e| e.kind == "impedance") { "equivalent" } else { "network" };
    Ok(Figure::Circuit(Circuit {
        version: SCHEMA_VERSION,
        flavor: flavor.into(),
        source: None,
        elements: l.elements,
        wires: l.wires,
        annotations: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_binds_tighter() {
        let n = parse("R_1 + R_2 || C_1").unwrap();
        assert_eq!(
            n,
            Net::Series(vec![
                Net::Leaf("R_1".into()),
                Net::Parallel(vec![Net::Leaf("R_2".into()), Net::Leaf("C_1".into())])
            ])
        );
    }

    #[test]
    fn subscripts_with_brackets_and_quotes_stay_in_the_operand() {
        let n = parse("R_(1 2) || C_\"load\" + j omega L").unwrap();
        let Net::Series(v) = n else { panic!() };
        assert_eq!(v[1], Net::Leaf("j omega L".into()));
        assert_eq!(v[0], Net::Parallel(vec![Net::Leaf("R_(1 2)".into()), Net::Leaf("C_\"load\"".into())]));
    }

    #[test]
    fn layout_places_parts_and_junctions() {
        let Figure::Circuit(c) = generate("R_1 + (R_2 || C_1)", &Map::new()).unwrap() else { panic!() };
        let kinds: Vec<_> = c.elements.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds.iter().filter(|k| **k == "node").count(), 2);
        let c1 = c.elements.iter().find(|e| e.label == "C_1").unwrap();
        assert_eq!(c1.kind, "capacitor");
        assert_eq!(c1.from, Some(Pt(4.0, -1.5)));
        assert_eq!(part_for("j omega L"), "impedance");
    }

    #[test]
    fn reports_bad_input() {
        assert!(parse("R_1 + (R_2").is_err());
        assert!(parse("R_1 | R_2").is_err());
        assert!(parse("").is_err());
    }
}
