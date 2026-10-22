//! `logic("y = (a and b) or not c")`: a boolean expression as a gate circuit.
//!
//! Operators, weakest first: or (`or | ∨ +`, `nor`), xor (`xor ⊕ ^`, `xnor`),
//! and (`and & ∧ · *`, `nand`), not (`not ~ ¬ !`). Chains of the same and/or/xor become
//! one gate with several inputs. `name = …` names the output (default `y`).
//! Every use of a variable gets its own input terminal.

use serde_json::{Map, Value};

use super::err;
use crate::SCHEMA_VERSION;
use crate::model::*;
use crate::parts;

#[derive(Clone, Debug, PartialEq)]
enum L {
    Var(String),
    Gate(&'static str, Vec<L>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Op(&'static str),
    Not,
    Open,
    Close,
    Name(String),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, Error> {
    let c: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        let single = match ch {
            '&' | '∧' | '·' | '*' => Some(Tok::Op("land")),
            '|' | '∨' | '+' => Some(Tok::Op("lor")),
            '^' | '⊕' => Some(Tok::Op("lxor")),
            '~' | '¬' | '!' => Some(Tok::Not),
            '(' => Some(Tok::Open),
            ')' => Some(Tok::Close),
            _ => None,
        };
        if let Some(t) = single {
            out.push(t);
            i += 1;
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' || ch == '"' {
            let mut w = String::new();
            while i < c.len() && (c[i].is_alphanumeric() || matches!(c[i], '_' | '"' | '.')) {
                if c[i] == '_' && c.get(i + 1) == Some(&'(') {
                    // Bracketed subscript: x_(1 2)
                    while i < c.len() && c[i] != ')' {
                        w.push(c[i]);
                        i += 1;
                    }
                }
                if i < c.len() {
                    w.push(c[i]);
                    i += 1;
                }
            }
            out.push(match w.as_str() {
                "and" | "AND" => Tok::Op("land"),
                "or" | "OR" => Tok::Op("lor"),
                "xor" | "XOR" => Tok::Op("lxor"),
                "nand" | "NAND" => Tok::Op("lnand"),
                "nor" | "NOR" => Tok::Op("lnor"),
                "xnor" | "XNOR" => Tok::Op("lxnor"),
                "not" | "NOT" => Tok::Not,
                _ => Tok::Name(w),
            });
            continue;
        }
        return err(format!("logic: unexpected '{ch}'"));
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn level(&mut self, ops: &[&'static str], next: fn(&mut Parser) -> Result<L, Error>) -> Result<L, Error> {
        let mut left = next(self)?;
        while let Some(Tok::Op(op)) = self.toks.get(self.pos) {
            let op = *op;
            if !ops.contains(&op) {
                break;
            }
            self.pos += 1;
            let right = next(self)?;
            left = match left {
                // and/or/xor chains merge into one gate; nand/nor/xnor do not associate.
                L::Gate(k, mut v) if k == op && matches!(op, "land" | "lor" | "lxor") => {
                    v.push(right);
                    L::Gate(k, v)
                }
                l => L::Gate(op, vec![l, right]),
            };
        }
        Ok(left)
    }
    fn or(&mut self) -> Result<L, Error> {
        self.level(&["lor", "lnor"], Parser::xor)
    }
    fn xor(&mut self) -> Result<L, Error> {
        self.level(&["lxor", "lxnor"], Parser::and)
    }
    fn and(&mut self) -> Result<L, Error> {
        self.level(&["land", "lnand"], Parser::unary)
    }
    fn unary(&mut self) -> Result<L, Error> {
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Not) => {
                self.pos += 1;
                Ok(L::Gate("lnot", vec![self.unary()?]))
            }
            Some(Tok::Name(n)) => {
                self.pos += 1;
                Ok(L::Var(n))
            }
            Some(Tok::Open) => {
                self.pos += 1;
                let e = self.or()?;
                if self.toks.get(self.pos) != Some(&Tok::Close) {
                    return err("logic: missing ')'");
                }
                self.pos += 1;
                Ok(e)
            }
            Some(t) => err(format!("logic: unexpected {t:?}")),
            None => err("logic: expression ends too early"),
        }
    }
}

fn parse(expr: &str) -> Result<(String, L), Error> {
    let (name, body) = match expr.split_once('=') {
        Some((n, b)) => (n.trim().to_string(), b),
        None => ("y".to_string(), expr),
    };
    let mut p = Parser { toks: tokenize(body)?, pos: 0 };
    let tree = p.or()?;
    if p.pos != p.toks.len() {
        return err(format!("logic: unexpected {:?}", p.toks[p.pos]));
    }
    Ok((name, tree))
}

const COL: f64 = 3.0;
const ROW: f64 = 1.0;

struct Out {
    elements: Vec<Element>,
    wires: Vec<Wire>,
    gates: usize,
    inputs: usize,
    next_row: usize,
}

fn level(t: &L) -> usize {
    match t {
        L::Var(_) => 0,
        L::Gate(_, v) => 1 + v.iter().map(level).max().unwrap_or(0),
    }
}

impl Out {
    /// Places `t`; returns its output point.
    fn place(&mut self, t: &L) -> Pt {
        match t {
            L::Var(name) => {
                self.inputs += 1;
                let at = Pt(0.0, -(self.next_row as f64) * ROW);
                self.next_row += 1;
                self.elements.push(Element {
                    id: format!("x{}", self.inputs),
                    kind: "terminal".into(),
                    at: Some(at),
                    label: name.clone(),
                    ..Default::default()
                });
                at
            }
            L::Gate(kind, children) => {
                let outs: Vec<Pt> = children.iter().map(|c| self.place(c)).collect();
                let y = outs.iter().map(|p| p.1).sum::<f64>() / outs.len() as f64;
                self.gates += 1;
                let mut el = Element {
                    id: format!("g{}", self.gates),
                    kind: (*kind).into(),
                    at: Some(Pt(level(t) as f64 * COL, y)),
                    ..Default::default()
                };
                if children.len() > 2 {
                    el.extra.insert("inputs".into(), Value::from(children.len()));
                }
                let pins = parts::pins(&el);
                for (k, from) in outs.iter().enumerate() {
                    let pin = pins.iter().find(|(n, _)| n == &format!("{}.in{}", el.id, k + 1)).unwrap().1;
                    let mid = pin.0 - 0.5;
                    let pts = if (from.1 - pin.1).abs() < 1e-9 {
                        vec![*from, pin]
                    } else {
                        vec![*from, Pt(mid, from.1), Pt(mid, pin.1), pin]
                    };
                    self.wires.push(Wire { points: pts.into_iter().map(WirePt::At).collect(), ..Default::default() });
                }
                let out = pins.iter().find(|(n, _)| n.ends_with(".out")).unwrap().1;
                self.elements.push(el);
                out
            }
        }
    }
}

pub fn generate(expr: &str, _o: &Map<String, Value>) -> Result<Figure, Error> {
    let (name, tree) = parse(expr)?;
    let mut o = Out { elements: vec![], wires: vec![], gates: 0, inputs: 0, next_row: 0 };
    let out = o.place(&tree);
    let end = Pt((level(&tree) as f64 + 0.6) * COL, out.1);
    o.elements.push(Element { id: "y".into(), kind: "terminal".into(), at: Some(end), label: name, ..Default::default() });
    o.wires.push(Wire { points: vec![WirePt::At(out), WirePt::At(end)], ..Default::default() });
    Ok(Figure::Circuit(Circuit {
        version: SCHEMA_VERSION,
        flavor: "logic".into(),
        source: None,
        elements: o.elements,
        wires: o.wires,
        annotations: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_chains() {
        let (_, t) = parse("a and b or not c").unwrap();
        assert_eq!(
            t,
            L::Gate(
                "lor",
                vec![L::Gate("land", vec![L::Var("a".into()), L::Var("b".into())]), L::Gate("lnot", vec![L::Var("c".into())])]
            )
        );
        let (_, t) = parse("a & b & c").unwrap();
        assert_eq!(t, L::Gate("land", vec![L::Var("a".into()), L::Var("b".into()), L::Var("c".into())]));
    }

    #[test]
    fn symbols_and_output_name() {
        let (name, t) = parse("s = x_1 ⊕ x_2").unwrap();
        assert_eq!(name, "s");
        assert_eq!(t, L::Gate("lxor", vec![L::Var("x_1".into()), L::Var("x_2".into())]));
    }

    #[test]
    fn layout_connects_every_input() {
        let Figure::Circuit(c) = generate("(a and b) or not c", &Map::new()).unwrap() else { panic!() };
        assert_eq!(c.elements.iter().filter(|e| e.kind == "terminal").count(), 4);
        assert_eq!(c.wires.len(), 6);
    }

    #[test]
    fn errors() {
        assert!(parse("a and").is_err());
        assert!(parse("(a or b").is_err());
        assert!(parse("a $ b").is_err());
    }
}
