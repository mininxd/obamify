use wasm_bindgen::prelude::*;
use crate::app::calculate::util::{GenerationSettings, get_images, ProgressSink};
use crate::app::preset::{Preset, UnprocessedPreset};
use rustc_hash::FxHasher as AHasher;
use pathfinding::prelude::Weights;
use crate::app::calculate::ProgressMsg;
use indexmap::IndexSet;

struct HeadlessProgressSink;
impl ProgressSink for HeadlessProgressSink {
    fn send(&mut self, _msg: ProgressMsg) {}
}

#[wasm_bindgen]
pub fn generate(source_image: JsValue, target_image: JsValue, settings: JsValue) -> Result<JsValue, JsValue> {
    let unprocessed: UnprocessedPreset = serde_wasm_bindgen::from_value(source_image).unwrap();
    let target: UnprocessedPreset = serde_wasm_bindgen::from_value(target_image).unwrap();
    let settings: GenerationSettings = serde_wasm_bindgen::from_value(settings).unwrap();

    let result = process_optimal(unprocessed, target, settings).map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(serde_wasm_bindgen::to_value(&result).unwrap())
}

#[inline(always)]
fn heuristic(
    apos: (u16, u16),
    bpos: (u16, u16),
    a: (u8, u8, u8),
    b: (u8, u8, u8),
    color_weight: i64,
    spatial_weight: i64,
) -> i64 {
    let spatial = (apos.0 as i64 - bpos.0 as i64).pow(2) + (apos.1 as i64 - bpos.1 as i64).pow(2);
    let color = (a.0 as i64 - b.0 as i64).pow(2)
        + (a.1 as i64 - b.1 as i64).pow(2)
        + (a.2 as i64 - b.2 as i64).pow(2);
    color * color_weight + (spatial * spatial_weight).pow(2)
}

struct ImgDiffWeights<'a> {
    source: Vec<(u8, u8, u8)>,
    target: Vec<(u8, u8, u8)>,
    weights: Vec<i64>,
    sidelen: usize,
    settings: &'a GenerationSettings,
}

impl Weights<i64> for ImgDiffWeights<'_> {
    fn rows(&self) -> usize {
        self.target.len()
    }

    fn columns(&self) -> usize {
        self.source.len()
    }

    #[inline(always)]
    fn at(&self, row: usize, col: usize) -> i64 {
        let (x1, y1) = (row % self.sidelen, row / self.sidelen);
        let (x2, y2) = (col % self.sidelen, col / self.sidelen);
        let (r1, g1, b1) = self.target[row];
        let (r2, g2, b2) = self.source[col];
        let weight = self.weights[row];
        -heuristic(
            (x1 as u16, y1 as u16),
            (x2 as u16, y2 as u16),
            (r1, g1, b1),
            (r2, g2, b2),
            weight,
            self.settings.proximity_importance,
        )
    }

    fn neg(&self) -> Self {
        todo!()
    }
}

type FxIndexSet<K> = IndexSet<K, std::hash::BuildHasherDefault<AHasher>>;

pub fn process_optimal(
    unprocessed: UnprocessedPreset,
    _target: UnprocessedPreset,
    settings: GenerationSettings,
) -> Result<Preset, Box<dyn std::error::Error>> {
    let source_img = image::ImageBuffer::from_vec(
        unprocessed.width,
        unprocessed.height,
        unprocessed.source_img.clone(),
    )
    .unwrap();
    let (source_pixels, target_pixels, weights) = get_images(source_img, &settings)?;

    let weights = ImgDiffWeights {
        source: source_pixels.clone(),
        target: target_pixels,
        weights,
        sidelen: settings.sidelen as usize,
        settings: &settings,
    };

    let (_total_diff, assignments) = {
        let nx = weights.rows();
        let ny = weights.columns();
        assert!(
            nx <= ny,
            "number of rows must not be larger than number of columns"
        );
        let mut xy: Vec<Option<usize>> = vec![None; nx];
        let mut yx: Vec<Option<usize>> = vec![None; ny];
        let mut lx: Vec<i64> = (0..nx)
            .map(|row| (0..ny).map(|col| weights.at(row, col)).max().unwrap())
            .collect::<Vec<_>>();
        let mut ly: Vec<i64> = vec![0; ny];
        let mut s = FxIndexSet::<usize>::default();
        let mut alternating = Vec::with_capacity(ny);
        let mut slack = vec![0; ny];
        let mut slackx = Vec::with_capacity(ny);
        for root in 0..nx {
            alternating.clear();
            alternating.resize(ny, None);
            let mut y = {
                s.clear();
                s.insert(root);
                for y in 0..ny {
                    slack[y] = lx[root] + ly[y] - weights.at(root, y);
                }
                slackx.clear();
                slackx.resize(ny, root);
                Some(loop {
                    let mut delta = pathfinding::num_traits::Bounded::max_value();
                    let mut x = 0;
                    let mut y = 0;
                    for yy in 0..ny {
                        if alternating[yy].is_none() && slack[yy] < delta {
                            delta = slack[yy];
                            x = slackx[yy];
                            y = yy;
                        }
                    }
                    if delta > 0 {
                        for &x in &s {
                            lx[x] -= delta;
                        }
                        for y in 0..ny {
                            if alternating[y].is_some() {
                                ly[y] += delta;
                            } else {
                                slack[y] -= delta;
                            }
                        }
                    }
                    alternating[y] = Some(x);
                    if yx[y].is_none() {
                        break y;
                    }
                    let x = yx[y].unwrap();
                    s.insert(x);
                    for y in 0..ny {
                        if alternating[y].is_none() {
                            let alternate_slack = lx[x] + ly[y] - weights.at(x, y);
                            if slack[y] > alternate_slack {
                                slack[y] = alternate_slack;
                                slackx[y] = x;
                            }
                        }
                    }
                })
            };
            while y.is_some() {
                let x = alternating[y.unwrap()].unwrap();
                let prec = xy[x];
                yx[y.unwrap()] = Some(x);
                xy[x] = y;
                y = prec;
            }
        }
        (
            lx.into_iter().sum::<i64>() + ly.into_iter().sum::<i64>(),
            xy.into_iter().map(Option::unwrap).collect::<Vec<_>>(),
        )
    };

    Ok(Preset {
        inner: UnprocessedPreset {
            name: unprocessed.name,
            width: settings.sidelen,
            height: settings.sidelen,
            source_img: source_pixels
                .into_iter()
                .flat_map(|(r, g, b)| [r, g, b, 255])
                .collect(),
        },
        assignments: assignments.clone(),
    })
}
