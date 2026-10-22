//! `magnetic("N_1 : N_2", fluxes: "split", ideal: true)`: transformers and magnetic circuits.
//!
//! Windings are separated by `:`; a leading `-` reverses a winding's sense. One winding
//! sits on the left leg, two on the left and right legs of a ring core. Port and flux
//! labels follow the winding label's subscript (`N_1` → `u_1`, `i_1`, `Psi_1`).

use serde_json::{Map, Value};

use super::{err, opt_bool, opt_str, subscript, with_sub};
use crate::SCHEMA_VERSION;
use crate::model::*;

struct Spec {
    label: String,
    /// true = cw: current into terminal 1 drives a clockwise flux.
    cw: bool,
}

fn parse(expr: &str) -> Result<Vec<Spec>, Error> {
    let mut out = vec![];
    for part in expr.split(':') {
        let part = part.trim();
        let (cw, label) = match part.strip_prefix('-') {
            Some(rest) => (false, rest.trim()),
            None => (true, part.strip_prefix('+').unwrap_or(part).trim()),
        };
        if label.is_empty() {
            return err(format!("magnetic: empty winding in '{expr}' (write e.g. \"N_1 : N_2\")"));
        }
        out.push(Spec { label: label.into(), cw });
    }
    if out.len() > 2 {
        return err("magnetic: at most two windings for now (three-leg cores are on the backlog)");
    }
    Ok(out)
}

fn cpt(leg: &str, pos: f64, lane: Option<f64>, flux: Option<&str>) -> CorePt {
    CorePt { leg: leg.into(), pos, lane, flux: flux.map(Into::into) }
}

fn callout(to: Target, rel: (f64, f64), content: impl Into<String>, tone: Option<&str>) -> Annotation {
    Annotation::Callout { to, rel: Pt(rel.0, rel.1), content: content.into(), tone: tone.map(Into::into) }
}

pub fn generate(expr: &str, o: &Map<String, Value>) -> Result<Figure, Error> {
    let specs = parse(expr)?;
    let two = specs.len() == 2;
    let fluxes_mode = opt_str(o, "fluxes").unwrap_or("main");
    if !matches!(fluxes_mode, "main" | "split" | "none") {
        return err(format!("magnetic: fluxes must be \"main\", \"split\" or \"none\", got \"{fluxes_mode}\""));
    }
    if fluxes_mode == "split" && !two {
        return err("magnetic: fluxes: \"split\" needs two windings");
    }
    let arrows = opt_str(o, "arrows").unwrap_or("consumer-generator");
    if !matches!(arrows, "consumer-generator" | "consumer") {
        return err("magnetic: arrows must be \"consumer-generator\" or \"consumer\"");
    }
    let gap = match o.get("gap") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Bool(true)) => Some("delta".to_string()),
        _ => None,
    };

    // Subscripts: from the labels, else numbered when there are two windings.
    let subs: Vec<Option<String>> = specs
        .iter()
        .enumerate()
        .map(|(k, s)| subscript(&s.label).map(String::from).or_else(|| two.then(|| (k + 1).to_string())))
        .collect();
    let sub = |k: usize| subs[k].as_deref();

    let legs = ["left", "right"];
    let mut windings = vec![];
    // Flux each winding's port current drives: its sense, reversed for generator arrows.
    let mut drives_cw = vec![];
    for (k, s) in specs.iter().enumerate() {
        let out = k == 1 && arrows == "consumer-generator";
        drives_cw.push(s.cw != out);
        windings.push(Winding {
            id: format!("w{}", k + 1),
            leg: legs[k].into(),
            sense: if s.cw { "cw" } else { "ccw" }.into(),
            turns: if two { 3 } else { 4 },
            label: s.label.clone(),
            span: None,
            port: Some(Port {
                i: Some(with_sub("i", sub(k))),
                i_dir: out.then(|| "out".into()),
                u: Some(with_sub("u", sub(k))),
                u_dir: None,
            }),
            extra: Map::new(),
        });
    }

    let mut fluxes = vec![];
    let mut sections = vec![];
    let mut notes = vec![];
    let dir = |cw: bool| if cw { "cw" } else { "ccw" }.to_string();
    match fluxes_mode {
        "main" => {
            fluxes.push(Flux {
                id: "f1".into(),
                path: "loop".into(),
                lane: 0.0,
                dir: dir(drives_cw[0]),
                arrows: vec![cpt("top", 0.6, None, None), cpt("bottom", 0.4, None, None)],
                ..Default::default()
            });
            let name = if two { "Phi_h" } else { "Phi" };
            notes.push(callout(Target::Core(cpt("top", 0.8, None, Some("f1"))), (0.6, 0.8), name, Some("magnetic")));
        }
        "split" => {
            let (s1, s2) = (sub(0).unwrap_or("1"), sub(1).unwrap_or("2"));
            let ideal = opt_bool(o, "ideal");
            // Φ_21: flux of winding 1 through winding 2 (outer lane); Φ_12 the reverse.
            fluxes.push(Flux {
                id: "f1".into(),
                path: "loop".into(),
                lane: 0.45,
                dir: dir(drives_cw[0]),
                arrows: vec![cpt("top", 0.75, None, None)],
                ..Default::default()
            });
            fluxes.push(Flux {
                id: "f2".into(),
                path: "loop".into(),
                lane: -0.3,
                dir: dir(drives_cw[1]),
                arrows: vec![cpt("bottom", 0.5, None, None)],
                ..Default::default()
            });
            for (k, leg) in legs.iter().enumerate() {
                fluxes.push(Flux {
                    id: format!("psi{}", k + 1),
                    path: (*leg).into(),
                    lane: 0.1,
                    dir: dir(specs[k].cw),
                    span: Some(Pt(0.15, 0.9)),
                    label: with_sub("Psi", sub(k)),
                    ..Default::default()
                });
            }
            let phi = |a: &str, b: &str| format!("Phi_({a}{b})");
            let l1 = if ideal { format!("{} = {}", phi(s2, s1), phi(s1, s1)) } else { phi(s2, s1) };
            let l2 = if ideal { format!("{} = {}", phi(s1, s2), phi(s2, s2)) } else { phi(s1, s2) };
            notes.push(callout(Target::Core(cpt("top", 0.6, None, Some("f1"))), (-1.0, 1.1), l1, Some("magnetic")));
            notes.push(callout(Target::Core(cpt("bottom", 0.4, None, Some("f2"))), (-0.5, -1.4), l2, Some("magnetic")));
            // Main flux, counted clockwise.
            sections.push(Section { id: "s1".into(), leg: "right".into(), pos: 0.96 });
            let term = |cw: bool, s: String, first: bool| match (cw, first) {
                (true, true) => s,
                (false, true) => format!("-{s}"),
                (true, false) => format!(" + {s}"),
                (false, false) => format!(" - {s}"),
            };
            let h = format!(
                "Phi_h = {}{}",
                term(drives_cw[0], phi(s2, s1), true),
                term(drives_cw[1], phi(s1, s2), false)
            );
            notes.push(callout(Target::Anchor("s1.north".into()), (0.4, 1.2), h, Some("magnetic")));
        }
        _ => {}
    }

    if opt_bool(o, "ideal") {
        for k in 0..specs.len() {
            let rel = if k == 0 { (-0.3, -0.9) } else { (0.5, -0.9) };
            notes.push(callout(Target::Anchor(format!("w{}.lead2", k + 1)), rel, format!("{} = 0", with_sub("R", sub(k))), None));
        }
        notes.push(callout(Target::Core(cpt("bottom", 0.85, Some(1.0), None)), (0.4, -0.9), "P_\"Fe\" = 0", None));
    }

    let mut core = if two {
        Core { shape: "ring".into(), size: Pt(6.0, 3.7), leg: 0.9, yoke: 0.6, label: "mu".into(), label_at: None, gaps: vec![] }
    } else {
        Core { shape: "ring".into(), size: Pt(5.0, 3.4), leg: 0.8, yoke: 0.8, label: "mu".into(), label_at: None, gaps: vec![] }
    };
    if let Some(g) = gap {
        let (leg, rel) = if two { ("top", (0.6, 0.8)) } else { ("right", (0.8, 0.6)) };
        let pos = if two { 0.65 } else { 0.5 };
        core.gaps.push(Gap { leg: leg.into(), pos, width: 0.25 });
        notes.push(callout(Target::Core(cpt(leg, pos, Some(1.0), None)), rel, g, None));
    }

    Ok(Figure::Magnetic(Magnetic {
        version: SCHEMA_VERSION,
        source: None,
        style: None,
        core,
        windings,
        fluxes,
        sections,
        annotations: notes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(expr: &str, opts: Value) -> Magnetic {
        match generate(expr, opts.as_object().unwrap()).unwrap() {
            Figure::Magnetic(m) => m,
            _ => unreachable!(),
        }
    }

    #[test]
    fn ideal_transformer_matches_the_slide() {
        let m = make("N_1 : N_2", serde_json::json!({"fluxes": "split", "ideal": true}));
        assert_eq!(m.windings[1].port.as_ref().unwrap().i_dir.as_deref(), Some("out"));
        assert_eq!(m.fluxes[0].dir, "cw");
        assert_eq!(m.fluxes[1].dir, "ccw");
        let labels: Vec<_> = m
            .annotations
            .iter()
            .map(|a| match a {
                Annotation::Callout { content, .. } => content.as_str(),
                _ => "",
            })
            .collect();
        assert!(labels.contains(&"Phi_(21) = Phi_(11)"));
        assert!(labels.contains(&"Phi_h = Phi_(21) - Phi_(12)"));
        assert!(labels.contains(&"R_2 = 0"));
    }

    #[test]
    fn minus_reverses_sense() {
        let m = make("N_1 : -N_2", serde_json::json!({}));
        assert_eq!(m.windings[1].sense, "ccw");
    }

    #[test]
    fn single_coil_with_gap() {
        let m = make("N", serde_json::json!({"gap": "delta"}));
        assert_eq!(m.windings.len(), 1);
        assert_eq!(m.windings[0].port.as_ref().unwrap().u.as_deref(), Some("u"));
        assert_eq!(m.core.gaps[0].leg, "right");
    }

    #[test]
    fn rejects_three_windings() {
        assert!(generate("N_1 : N_2 : N_3", &Map::new()).is_err());
    }
}
