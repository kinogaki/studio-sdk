//! [`Curves::from_polylines`]: a [`Polygons`] value's polylines as linear
//! curves, sharing its buffers.
use super::{Basis, Curves};
use crate::attributes::{Attributes, Class};
use crate::column::Column;
use crate::polygons::Polygons;
use std::sync::Arc;

impl Curves {
    /// The polylines of `polygons` as linear curves, without copying their
    /// points or vertices: the curves share the positions column, and the
    /// point index is a view of the polylines' part of the vertices.
    ///
    /// Point and Detail attributes and groups are shared as they are;
    /// Vertex and Primitive ones are views of the polylines' part (a
    /// Primitive value missing is read from Piece, as every primitive
    /// reader does). Edge attributes and groups don't carry over. Only the
    /// offsets (one a curve), the degrees and the Primitive groups' bits
    /// are new.
    ///
    /// ```
    /// use kinogaki_geometry::{Attributes, Curves, Polygons, Topology};
    ///
    /// // A triangle, then a polyline through two of its points and a third.
    /// let t = Topology::new(4, vec![0, 3, 6].into(), vec![0, 1, 2, 1, 2, 3].into(), 1).unwrap();
    /// let polygons = Polygons::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]].into(), t, Attributes::new()).unwrap();
    /// let curves = Curves::from_polylines(&polygons);
    /// assert_eq!(curves.curves(), 1);
    /// assert!(curves.positions().ptr_eq(polygons.positions()));
    /// assert_eq!(&**curves.points().unwrap(), &[1, 2, 3]);
    /// ```
    pub fn from_polylines(polygons: &Polygons) -> Curves {
        let topology = polygons.topology();
        let lines = topology.polylines();
        let first_vertex = topology.polygon_vertices();
        let vertex_range = first_vertex..topology.vertex_count();
        let offsets: Column<u32> = topology.offsets()[lines.start..]
            .iter()
            .map(|&o| o - first_vertex as u32)
            .collect();
        let points = topology.vertices().slice(vertex_range.clone());
        let source = polygons.attributes();
        let mut attributes = Attributes::new();
        let mut add = |class: Class, name: &str, attribute| {
            attributes = attributes
                .with_shared(class, name, attribute)
                .expect("a checked attribute stays valid");
        };
        for (class, name, attribute) in source.iter() {
            match class {
                Class::Point | Class::Detail => add(
                    class,
                    name,
                    source.get_shared(class, name).expect("listed").clone(),
                ),
                Class::Vertex => add(class, name, Arc::new(attribute.slice(vertex_range.clone()))),
                Class::Primitive => add(class, name, Arc::new(attribute.slice(lines.clone()))),
                Class::Piece if source.get(Class::Primitive, name).is_none() => add(
                    Class::Primitive,
                    name,
                    Arc::new(attribute.slice(lines.clone())),
                ),
                _ => {}
            }
        }
        for (class, name, group) in source.groups() {
            let (class, group) = match class {
                Class::Point | Class::Detail => (class, group.clone()),
                Class::Vertex => (class, group.slice(vertex_range.clone())),
                Class::Primitive => (class, group.slice(lines.clone())),
                Class::Piece if source.group(Class::Primitive, name).is_none() => {
                    (Class::Primitive, group.slice(lines.clone()))
                }
                _ => continue,
            };
            attributes = attributes
                .with_group(class, name, group)
                .expect("a checked group stays valid");
        }
        Curves::builder(Basis::Linear, polygons.positions().clone(), offsets)
            .points(points)
            .attributes(attributes)
            .build()
            .expect("a checked value's polylines are valid curves")
    }
}
