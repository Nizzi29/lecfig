//! `loop("feedback(C G, H)")`: block algebra as a block diagram.
//!
//! - `A B` (space) is a series connection, `A + B` / `A - B` a parallel one,
//! - `feedback(F, H)` closes a negative loop around F through H, `feedback(F)` a unity
//!   loop, `feedback(F, H, +)` a positive one; loops nest,
//! - blocks are Typst math names (`G_1`, `K_p`, `G(s)`) or literal content in brackets
//!   (`[K/(T s + 1)]`).
//!
//! Options: `input` (default `r`), `output` (default `y`) name the signals.

use serde_json::{Map, Value};

use super::{err, opt_str};
use crate::model::*;

#[derive(Clone, Debug, PartialEq)]
enum B {
    Block(String),
    Series(Vec<B>),
    /// Terms with their sign at the sum.
    Parallel(Vec<(bool, B)>),
    Feedback { fwd: Box<B>, fb: Option<Box<B>>, positive: bool },
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i] == ' ' {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn sum(&mut self) -> Result<B, Error> {
        let mut terms = vec![(true, self.prod()?)];
        while let Some(c @ ('+' | '-')) = self.peek() {
            self.i += 1;
            terms.push((c == '+', self.prod()?));
        }
        Ok(if terms.len() == 1 { terms.pop().unwrap().1 } else { B::Parallel(terms) })
    }
    fn prod(&mut self) -> Result<B, Error> {
        let mut items = vec![self.factor()?];
        while let Some(c) = self.peek() {
            if matches!(c, '+' | '-' | ')' | ',') {
                break;
            }
            items.push(self.factor()?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            B::Series(items.into_iter().flat_map(|b| if let B::Series(v) = b { v } else { vec![b] }).collect())
        })
    }
    fn factor(&mut self) -> Result<B, Error> {
        match self.peek() {
            Some('(') => {
                self.i += 1;
                let e = self.sum()?;
                self.expect(')')?;
                Ok(e)
            }
            Some('[') => {
                self.i += 1;
                let start = self.i;
                let mut depth = 1;
                while self.i < self.s.len() {
                    match self.s[self.i] {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                if self.i >= self.s.len() {
                    return err("loop: missing ']'");
                }
                let text: String = self.s[start..self.i].iter().collect();
                self.i += 1;
                Ok(B::Block(text.trim().to_string()))
            }
            Some(c) if c.is_alphanumeric() || c == '_' => {
                let start = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_alphanumeric() || matches!(self.s[self.i], '_' | '^' | '"' | '.')) {
                    // Bracketed subscripts / exponents belong to the name: K_(p 1).
                    if matches!(self.s[self.i], '_' | '^') && self.s.get(self.i + 1) == Some(&'(') {
                        self.i += 1;
                        self.skip_group();
                        continue;
                    }
                    self.i += 1;
                }
                let name: String = self.s[start..self.i].iter().collect();
                if name == "feedback" && self.peek() == Some('(') {
                    self.i += 1;
                    let fwd = self.sum()?;
                    let mut fb = None;
                    let mut positive = false;
                    if self.peek() == Some(',') {
                        self.i += 1;
                        fb = Some(Box::new(self.sum()?));
                        if self.peek() == Some(',') {
                            self.i += 1;
                            match self.peek() {
                                Some('+') => positive = true,
                                Some('-') => {}
                                _ => return err("loop: the third argument of feedback is + or -"),
                            }
                            self.i += 1;
                        }
                    }
                    self.expect(')')?;
                    return Ok(B::Feedback { fwd: Box::new(fwd), fb, positive });
                }
                // `G(s)` right after a name is part of it.
                if self.s.get(self.i) == Some(&'(') {
                    let save = self.i;
                    self.skip_group();
                    let group: String = self.s[save..self.i].iter().collect();
                    if group.len() > 5 {
                        return err(format!("loop: write complex blocks in brackets, e.g. [{name}{group}]"));
                    }
                    return Ok(B::Block(format!("{name}{group}")));
                }
                Ok(B::Block(name))
            }
            Some(c) => err(format!("loop: unexpected '{c}'")),
            None => err("loop: expression ends too early"),
        }
    }
    fn skip_group(&mut self) {
        let mut depth = 0;
        while self.i < self.s.len() {
            match self.s[self.i] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        self.i += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
    }
    fn expect(&mut self, c: char) -> Result<(), Error> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            err(format!("loop: expected '{c}'"))
        }
    }
}

fn parse(expr: &str) -> Result<B, Error> {
    let mut p = Parser { s: expr.chars().collect(), i: 0 };
    let b = p.sum()?;
    if p.peek().is_some() {
        return err(format!("loop: unexpected '{}'", p.peek().unwrap()));
    }
    Ok(b)
}

/// A placed subgraph: entry and exit node ids, size in cells.
struct Placed {
    entry: String,
    exit: String,
    width: f64,
    /// Rows used below the subgraph's main row.
    depth: f64,
}

struct Out {
    nodes: Vec<BNode>,
    edges: Vec<BEdge>,
    n: usize,
}

impl Out {
    fn node(&mut self, kind: &str, at: Pt, label: &str) -> String {
        self.n += 1;
        let id = format!("{}{}", &kind[..1], self.n);
        self.nodes.push(BNode { id: id.clone(), kind: kind.into(), at, label: label.into(), extra: Map::new() });
        id
    }
    /// Every input of a sum shows its sign; unsigned ones are `+`.
    fn edge(&mut self, from: &str, to: &str, sign: &str, bends: Vec<Pt>) {
        let into_sum = self.nodes.iter().any(|n| n.id == to && n.kind == "sum");
        let sign = if into_sum && sign.is_empty() { "+" } else { sign };
        self.edges.push(BEdge { from: from.into(), to: to.into(), sign: sign.into(), bends, ..Default::default() });
    }

    fn width(b: &B) -> f64 {
        match b {
            B::Block(_) => 1.0,
            B::Series(v) => v.iter().map(Self::width).sum(),
            B::Parallel(v) => v.iter().map(|(_, t)| Self::width(t)).fold(0.0, f64::max) + 2.0,
            B::Feedback { fwd, .. } => Self::width(fwd) + 2.0,
        }
    }

    /// Places `b` with its main row at `row`, starting at column `col`. `mirror` lays a
    /// series out right to left (for feedback paths).
    fn place(&mut self, b: &B, col: f64, row: f64, mirror: bool) -> Placed {
        match b {
            B::Block(label) => {
                let id = self.node("tf", Pt(col, row), label);
                Placed { entry: id.clone(), exit: id, width: 1.0, depth: 0.0 }
            }
            B::Series(items) => {
                let total = Self::width(b);
                let mut x = if mirror { col + total } else { col };
                let mut placed: Vec<Placed> = vec![];
                for it in items {
                    let w = Self::width(it);
                    let start = if mirror { x - w } else { x };
                    placed.push(self.place(it, start, row, mirror));
                    x = if mirror { x - w } else { x + w };
                }
                for k in 1..placed.len() {
                    let (a, b) = (placed[k - 1].exit.clone(), placed[k].entry.clone());
                    self.edge(&a, &b, "", vec![]);
                }
                let depth = placed.iter().map(|p| p.depth).fold(0.0, f64::max);
                Placed { entry: placed[0].entry.clone(), exit: placed.last().unwrap().exit.clone(), width: total, depth }
            }
            B::Parallel(terms) => {
                let inner = Self::width(b) - 2.0;
                let br = self.node("branch", Pt(col, row), "");
                let sum_col = col + 1.0 + inner;
                let sum = self.node("sum", Pt(sum_col, row), "");
                let mut r = row;
                let mut depth = 0.0;
                for (k, (plus, t)) in terms.iter().enumerate() {
                    let w = Self::width(t);
                    let p = self.place(t, col + 1.0 + (inner - w) / 2.0, r, false);
                    let sign = if *plus { "+" } else { "-" };
                    if k == 0 {
                        self.edge(&br, &p.entry, "", vec![]);
                        self.edge(&p.exit, &sum, sign, vec![]);
                    } else {
                        self.edge(&br, &p.entry, "", vec![Pt(col, r)]);
                        self.edge(&p.exit, &sum, sign, vec![Pt(sum_col, r)]);
                    }
                    depth = r - row + p.depth;
                    r += p.depth + 1.0;
                }
                Placed { entry: br, exit: sum, width: inner + 2.0, depth }
            }
            B::Feedback { fwd, fb, positive } => {
                let sum = self.node("sum", Pt(col, row), "");
                let f = self.place(fwd, col + 1.0, row, false);
                let br_col = col + 1.0 + f.width;
                let br = self.node("branch", Pt(br_col, row), "");
                self.edge(&sum, &f.entry, "", vec![]);
                self.edge(&f.exit, &br, "", vec![]);
                let sign = if *positive { "+" } else { "-" };
                let fb_row = row + f.depth + 1.0;
                let depth = match fb {
                    Some(h) => {
                        let w = Self::width(h);
                        let p = self.place(h, col + 1.0 + (f.width - w) / 2.0, fb_row, true);
                        self.edge(&br, &p.entry, "", vec![Pt(br_col, fb_row)]);
                        self.edge(&p.exit, &sum, sign, vec![Pt(col, fb_row)]);
                        fb_row - row + p.depth
                    }
                    None => {
                        self.edge(&br, &sum, sign, vec![Pt(br_col, fb_row), Pt(col, fb_row)]);
                        fb_row - row
                    }
                };
                Placed { entry: sum, exit: br, width: f.width + 2.0, depth }
            }
        }
    }
}

pub fn generate(expr: &str, o: &Map<String, Value>) -> Result<Figure, Error> {
    let tree = parse(expr)?;
    let mut out = Out { nodes: vec![], edges: vec![], n: 0 };
    let input = out.node("io", Pt(0.0, 0.0), opt_str(o, "input").unwrap_or("r"));
    let p = out.place(&tree, 1.0, 0.0, false);
    let output = out.node("io", Pt(1.0 + p.width, 0.0), opt_str(o, "output").unwrap_or("y"));
    out.edge(&input, &p.entry, "", vec![]);
    out.edge(&p.exit, &output, "", vec![]);
    Ok(Figure::Block(Block { nodes: out.nodes, edges: out.edges, ..Default::default() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_loops() {
        let b = parse("feedback(K feedback(G_1, H_1), H_2)").unwrap();
        let B::Feedback { fwd, fb, positive } = b else { panic!() };
        assert!(!positive);
        assert_eq!(fb.unwrap().as_ref(), &B::Block("H_2".into()));
        let B::Series(v) = *fwd else { panic!() };
        assert_eq!(v[0], B::Block("K".into()));
        assert!(matches!(v[1], B::Feedback { .. }));
    }

    #[test]
    fn literal_blocks_and_transfer_functions() {
        assert_eq!(parse("[K/(T s + 1)] G(s)").unwrap(), B::Series(vec![B::Block("K/(T s + 1)".into()), B::Block("G(s)".into())]));
        assert_eq!(parse("G_1 - G_2").unwrap(), B::Parallel(vec![(true, B::Block("G_1".into())), (false, B::Block("G_2".into()))]));
    }

    #[test]
    fn standard_loop_layout() {
        let Figure::Block(b) = generate("feedback(C G, H)", &Map::new()).unwrap() else { panic!() };
        let labels: Vec<_> = b.nodes.iter().map(|n| (n.kind.as_str(), n.label.as_str())).collect();
        assert!(labels.contains(&("tf", "H")));
        let h = b.nodes.iter().find(|n| n.label == "H").unwrap();
        assert_eq!(h.at.1, 1.0, "feedback below the forward path");
        assert!(b.edges.iter().any(|e| e.sign == "-"));
        assert!(b.edges.iter().any(|e| e.sign == "+"));
    }

    #[test]
    fn unity_feedback_and_errors() {
        assert!(generate("feedback(G)", &Map::new()).is_ok());
        assert!(parse("feedback(G, H, *)").is_err());
        assert!(parse("(G H").is_err());
    }
}
