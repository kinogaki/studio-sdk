//! What a host does to an engine over time, beside one scene's final frame:
//! a final of an empty scene; one through a camera not in the scene, which
//! fails at once and says why; an edit after the first frame; a view id given
//! again at another size; a scene of 6.4 million triangles, rendered or failed
//! saying why, never a read let go unanswered; and the nodes adoption cut
//! from a material, each said.
use super::scenes::{camera, dome, node, quad};
use super::{Make, SIDE, Session, VIEW, alike, beauty, finite_beauty, id};
use kinogaki_render_api::geometry::{Column, Error};
use kinogaki_render_api::{
    Change, Dirty, EngineDescription, Engines, Id, Material, MaterialNetwork, Prim, SceneUpdate,
    Shape, Subsets, Topology, ViewCamera,
};
use std::sync::Arc;

/// Each check's name and outcome.
pub(super) fn checks(
    make: Make<'_>,
    description: &EngineDescription,
) -> Vec<(&'static str, Result<(), String>)> {
    vec![
        ("an empty scene", empty(make)),
        ("a view through a camera not in the scene", no_camera(make)),
        ("an edit after the first frame", edited(make, description)),
        ("a view id given again", reused(make, description)),
        ("a scene of 6.4 million triangles", large(make)),
        ("the nodes adoption cut, said", cut(make, description)),
    ]
}

/// The conformance camera, as a view's.
fn through() -> ViewCamera {
    ViewCamera::Free(camera())
}

/// The quad, prim 0, lit by the dome where the engine declares one.
fn lit_quad(description: &EngineDescription) -> Vec<Prim> {
    let quad = Prim::Shape(quad(1.0));
    std::iter::once(quad).chain(dome(description)).collect()
}

/// A final read's failure as a check sees it: one that says why passes, one
/// the engine let go unanswered (its render thread gone) doesn't.
fn said_why(session: &Session, why: Error) -> Result<(), String> {
    if why.is_cancelled() {
        return Err(format!("it let the read go without saying why ({why})"));
    }
    match session.engine.problem(VIEW) {
        Some(_) => Ok(()),
        None => Err(format!("its read failed ({why}) and its view says nothing")),
    }
}

/// An empty scene renders: an Image of the view's size, finite.
fn empty(make: Make<'_>) -> Result<(), String> {
    let mut session = Session::new(make)?;
    session.send(Vec::new());
    let outputs = session.render(through(), [SIDE; 2]);
    finite_beauty(&outputs.map_err(|e| e.to_string())?, [SIDE; 2]).map(drop)
}

/// A final through a camera not in the scene fails at once, saying why.
fn no_camera(make: Make<'_>) -> Result<(), String> {
    let mut session = Session::new(make)?;
    session.send(vec![Prim::Shape(quad(1.0))]);
    match session.render(ViewCamera::Prim(Id::of(id(99))), [SIDE; 2]) {
        Ok(_) => Err("it rendered a frame through no camera".into()),
        Err(why) => said_why(&session, why),
    }
}

/// After a first frame, the quad moved out of sight (its points changed)
/// renders as a fresh engine renders the scene without it.
fn edited(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let mut session = Session::new(make)?;
    session.send(lit_quad(description));
    session
        .render(through(), [SIDE; 2])
        .map_err(|e| e.to_string())?;
    let near = quad(1.0);
    let far: Vec<[f32; 3]> = (near.points().iter())
        .map(|p| [p[0] + 1000.0, p[1], p[2]])
        .collect();
    let away = (near.rebuilt().points(Column::from(far)).build()).expect("a quad far away");
    session.engine.update(SceneUpdate {
        changed: vec![Change::new(id(0), Dirty::POINTS, Prim::Shape(away))],
        ..SceneUpdate::new()
    });
    let after = session
        .render(through(), [SIDE; 2])
        .map_err(|e| e.to_string())?;
    let after = finite_beauty(&after, [SIDE; 2])?;
    let without = beauty(make, dome(description).into_iter().collect())?;
    alike(&after, &without)
}

/// A view dropped and its id given again at another size renders at that
/// size, as a fresh engine renders it: never the old view's frame.
fn reused(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let size = [SIDE, SIDE / 2];
    let render = |session: &mut Session| {
        let outputs = session.render(through(), size).map_err(|e| e.to_string())?;
        finite_beauty(&outputs, size)
    };
    let mut session = Session::new(make)?;
    session.send(lit_quad(description));
    session
        .render(through(), [SIDE; 2])
        .map_err(|e| e.to_string())?;
    session.engine.drop_view(VIEW);
    let again = render(&mut session)?;
    let mut fresh = Session::new(make)?;
    fresh.send(lit_quad(description));
    alike(&again, &render(&mut fresh)?)
}

/// Quads a side of the large scene's grid: 6,422,528 triangles.
const LARGE: u32 = 1792;

/// A scene of 6.4 million triangles in one grid: it renders, or its read
/// fails saying why (past what a device's buffers hold).
fn large(make: Make<'_>) -> Result<(), String> {
    let mut session = Session::new(make)?;
    session.send(vec![Prim::Shape(grid(LARGE))]);
    match session.render(through(), [4, 4]) {
        Ok(outputs) => finite_beauty(&outputs, [4, 4]).map(drop),
        Err(why) => said_why(&session, why),
    }
}

/// A grid of `n` × `n` quads over the quad's square, two triangles each.
fn grid(n: u32) -> Shape {
    let side = n + 1;
    let at = |i: u32| -1.0 + 2.0 * i as f32 / n as f32;
    let points: Vec<[f32; 3]> = (0..side * side)
        .map(|p| [at(p % side), at(p / side), 0.0])
        .collect();
    let triangles: Vec<[u32; 3]> = (0..n * n)
        .flat_map(|q| {
            let c = q / n * side + q % n;
            [[c, c + 1, c + side + 1], [c, c + side + 1, c + side]]
        })
        .collect();
    Shape::builder(points.into(), Topology::Triangles(triangles.into()))
        .path("/grid")
        .build()
        .expect("a whole grid")
}

/// A material whose base color a hextiled image drives: each node adoption
/// cut from it for the engine (none, for an engine rendering them) is said,
/// named, once the material renders.
fn cut(make: Make<'_>, description: &EngineDescription) -> Result<(), String> {
    let mut built = MaterialNetwork::builder();
    let surface = node(&mut built, "ND_open_pbr_surface_surfaceshader", &[]);
    let tiled = node(&mut built, "ND_hextiledimage_color3", &[]);
    built
        .link(tiled, "out", surface, "base_color")
        .expect("a color");
    let shown = node(&mut built, "ND_surfacematerial", &[]);
    (built.link(surface, "out", shown, "surfaceshader")).expect("a surface");
    let network = built.build(shown).expect("a whole material");
    let network = Engines::new(vec![description.clone()]).adopt(&network, description.id());
    let cut = network.cut().to_vec();
    let material = Prim::Material(Material {
        network: Arc::new(network),
        path: "/tiled".into(),
        double_sided: true,
    });
    let slots = Subsets::new(vec![0u16; 2].into(), vec![Id::of(id(0))]);
    let shape = (quad(1.0).rebuilt()).subsets(slots).build();
    let mut session = Session::new(make)?;
    session.send(vec![material, Prim::Shape(shape.expect("one slot"))]);
    session
        .render(through(), [SIDE; 2])
        .map_err(|e| e.to_string())?;
    let said = session.engine.said();
    let unsaid: Vec<_> = (cut.iter())
        .filter(|node| !said.iter().any(|d| d.message.contains(&***node)))
        .collect();
    match unsaid.is_empty() {
        true => Ok(()),
        false => Err(format!(
            "it says nothing of {unsaid:?}, which it doesn't render"
        )),
    }
}
