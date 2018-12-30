//! Code for resizing, scaling and fitting polylines.
use svg2polylines::Polyline;


#[derive(Debug, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    pub fn spread(&self) -> f64 {
        self.max - self.min
    }
}

#[derive(Debug, PartialEq)]
pub struct Bounds {
    pub x: Range,
    pub y: Range,
}

impl Bounds {
    /// Add padding. Panic if this results in min <= max.
    pub fn add_padding(&mut self, padding: f64) {
        self.x.min += padding;
        self.x.max -= padding;
        self.y.min += padding;
        self.y.max -= padding;
        assert!(self.x.spread() >= 0.0);
        assert!(self.y.spread() >= 0.0);
    }
}

/// Get the bounds (maxima / minima) of the specified polylines.
fn get_bounds(polylines: &Vec<Polyline>) -> Option<Bounds> {
    let mut x_min = None;
    let mut x_max = None;
    let mut y_min = None;
    let mut y_max = None;
    for polyline in polylines {
        for coord in polyline {
            match x_min {
                None => x_min = Some(coord.x),
                Some(x) if coord.x < x => x_min = Some(coord.x),
                Some(_) => {},
            }
            match x_max {
                None => x_max = Some(coord.x),
                Some(x) if coord.x > x => x_max = Some(coord.x),
                Some(_) => {},
            }
            match y_min {
                None => y_min = Some(coord.y),
                Some(y) if coord.y < y => y_min = Some(coord.y),
                Some(_) => {},
            }
            match y_max {
                None => y_max = Some(coord.y),
                Some(y) if coord.y > y => y_max = Some(coord.y),
                Some(_) => {},
            }
        }
    }
    match (x_min, x_max, y_min, y_max) {
        (Some(x_min), Some(x_max), Some(y_min), Some(y_max)) => {
            Some(Bounds {
                x: Range { min: x_min, max: x_max },
                y: Range { min: y_min, max: y_max },
            })
        },
        _ => None,
    }
}

#[inline]
fn partial_min<T: PartialOrd>(v1: T, v2: T) -> T {
    if v1 <= v2 { v1 } else { v2 }
}

/// Scale polylines using the specified scaling factor.
pub fn scale_polylines(polylines: &mut Vec<Polyline>, offset: (f64, f64), scale: (f64, f64)) {
    info!("Scaling polylines with offset {:?} and scale factor {:?}", offset, scale);
    for polyline in polylines {
        for coord in polyline {
            coord.x = scale.0 * coord.x + offset.0;
            coord.y = scale.1 * coord.y + offset.1;
        }
    }
}

/// Fit polylines within the specified bounds.
pub fn fit_polylines(polylines: &mut Vec<Polyline>, target_bounds: &Bounds) -> Result<(), String> {
    info!("Fitting polylines into specified bounds");

    // Handle empty polylines
    if polylines.is_empty() {
        warn!("Emtpy polylines");
        return Ok(());
    }

    // Calculate current bounds
    let current_bounds = get_bounds(&polylines)
        .ok_or("Could not calculate bounds".to_string())?;

    // Calculate scale factor
    let x_factor = target_bounds.x.spread() / current_bounds.x.spread();
    let y_factor = target_bounds.y.spread() / current_bounds.y.spread();
    let scale_factor = partial_min(
        // Handle zero, infinite, subnormal and NaN values
        if x_factor.is_normal() { x_factor } else { 1.0 },
        if y_factor.is_normal() { y_factor } else { 1.0 },
    );

    // Calculate offset for horizontal centering
    let width = current_bounds.x.spread() * scale_factor;
    let x_offset = (target_bounds.x.spread() - width) / 2.0;

    // Translate and scale
    for polyline in polylines {
        for coord in polyline {
            coord.x = (coord.x - current_bounds.x.min) * scale_factor + target_bounds.x.min + x_offset;
            coord.y = (coord.y - current_bounds.y.min) * scale_factor + target_bounds.y.min;
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use svg2polylines::CoordinatePair;

    use super::*;

    #[test]
    fn test_get_bounds_empty() {
        let polylines = vec![];
        assert!(get_bounds(&polylines).is_none()); 
    }

    #[test]
    fn test_get_bounds_1() {
        let polylines = vec![
            vec![
                CoordinatePair { x: 1.0, y: 1.0 },
                CoordinatePair { x: 2.0, y: 2.0 },
                CoordinatePair { x: 0.0, y: 1.5 },
            ],
        ];
        assert_eq!(get_bounds(&polylines).unwrap(), Bounds {
            x: Range { min: 0.0, max: 2.0 },
            y: Range { min: 1.0, max: 2.0 },
        }); 
    }

    #[test]
    fn test_get_bounds_2() {
        let polylines = vec![
            vec![
                CoordinatePair { x: 1.0, y: 2.0 },
                CoordinatePair { x: 2.0, y: 1.0 },
            ],
            vec![
                CoordinatePair { x: 3.0, y: -1.0 },
                CoordinatePair { x: 2.0, y: 1.0 },
            ],
        ];
        assert_eq!(get_bounds(&polylines).unwrap(), Bounds {
            x: Range { min: 1.0, max: 3.0 },
            y: Range { min: -1.0, max: 2.0 },
        }); 
    }

    #[test]
    fn test_fit_polylines() {
        let mut polylines = vec![
            vec![
                CoordinatePair { x: 2.0, y: 2.0 },
                CoordinatePair { x: 5.0, y: 8.0 },
            ],
            vec![
                CoordinatePair { x: 2.0, y: 5.0 },
                CoordinatePair { x: 5.0, y: 5.0 },
            ],
        ];
        let target_bounds = Bounds {
            x: Range { min: 1.0, max: 4.0 },
            y: Range { min: 1.0, max: 3.0 },
        };
        fit_polylines(&mut polylines, &target_bounds).unwrap();
        assert_eq!(polylines.len(), 2);
        assert_eq!(polylines[0], vec![
            CoordinatePair { x: 2.0, y: 1.0 },
            CoordinatePair { x: 3.0, y: 3.0 },
        ]);
        assert_eq!(polylines[1], vec![
            CoordinatePair { x: 2.0, y: 2.0 },
            CoordinatePair { x: 3.0, y: 2.0 },
        ]);
    }

    #[test]
    fn test_fit_polylines_single_point() {
        let mut polylines = vec![
            vec![
                CoordinatePair { x: 7.0, y: 12.0 },
            ],
        ];
        let target_bounds = Bounds {
            x: Range { min: 1.0, max: 4.0 },
            y: Range { min: 1.0, max: 3.0 },
        };
        fit_polylines(&mut polylines, &target_bounds).unwrap();
        assert_eq!(polylines, vec![vec![CoordinatePair { x: 2.5, y: 1.0 }]]);
    }
}
