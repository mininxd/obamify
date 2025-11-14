use wasm_bindgen::prelude::*;
use crate::app::calculate::util::{GenerationSettings, get_images, ProgressSink, Algorithm};
use crate::app::preset::{Preset, UnprocessedPreset};
use rustc_hash::FxHasher as AHasher;
use pathfinding::prelude::Weights;
use indexmap::IndexSet;
use crate::app::calculate::{ProgressMsg, heuristic, Pixel, SWAPS_PER_GENERATION_PER_PIXEL};
use frand::Rand;

struct HeadlessProgressSink;
impl ProgressSink for HeadlessProgressSink {
    fn send(&mut self, _msg: ProgressMsg) {}
}

#[wasm_bindgen]
pub struct Generator {
    settings: GenerationSettings,
}

#[wasm_bindgen]
impl Generator {
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<Generator, JsValue> {
        let settings: GenerationSettings = serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Generator { settings })
    }

    pub fn generate(&self, source_image: JsValue, target_image: JsValue) -> Result<JsValue, JsValue> {
        let unprocessed: UnprocessedPreset = serde_wasm_bindgen::from_value(source_image)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let _target: UnprocessedPreset = serde_wasm_bindgen::from_value(target_image)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let result = match self.settings.algorithm {
            Algorithm::Optimal => self.process_optimal(unprocessed),
            Algorithm::Genetic => self.process_fast(unprocessed),
        }
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(serde_wasm_bindgen::to_value(&result).unwrap())
    }
}

impl Generator {
    fn process_optimal(&self, unprocessed: UnprocessedPreset) -> Result<Preset, Box<dyn std::error::Error>> {
        let source_img = image::ImageBuffer::from_vec(
            unprocessed.width,
            unprocessed.height,
            unprocessed.source_img.clone(),
        )
        .unwrap();
        let (source_pixels, target_pixels, weights) = get_images(source_img, &self.settings)?;

        let weights_struct = ImgDiffWeights {
            source: source_pixels.clone(),
            target: target_pixels,
            weights,
            sidelen: self.settings.sidelen as usize,
            settings: &self.settings,
        };

        let (_total_diff, assignments) = {
            let nx = weights_struct.rows();
            let ny = weights_struct.columns();
            assert!(
                nx <= ny,
                "number of rows must not be larger than number of columns"
            );
            let mut xy: Vec<Option<usize>> = vec![None; nx];
            let mut yx: Vec<Option<usize>> = vec![None; ny];
            let mut lx: Vec<i64> = (0..nx)
                .map(|row| (0..ny).map(|col| weights_struct.at(row, col)).max().unwrap())
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
                        slack[y] = lx[root] + ly[y] - weights_struct.at(root, y);
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
                                let alternate_slack = lx[x] + ly[y] - weights_struct.at(x, y);
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
                width: self.settings.sidelen,
                height: self.settings.sidelen,
                source_img: source_pixels
                    .into_iter()
                    .flat_map(|(r, g, b)| [r, g, b, 255])
                    .collect(),
            },
            assignments,
        })
    }

    fn process_fast(&self, unprocessed: UnprocessedPreset) -> Result<Preset, Box<dyn std::error::Error>> {
        let source_img = image::ImageBuffer::from_vec(
            unprocessed.width,
            unprocessed.height,
            unprocessed.source_img.clone(),
        )
        .unwrap();
        let (source_pixels, target_pixels, weights) = get_images(source_img, &self.settings)?;

        let mut pixels = source_pixels
            .iter()
            .enumerate()
            .map(|(i, &(r, g, b))| {
                let x = (i as u32 % self.settings.sidelen) as u16;
                let y = (i as u32 / self.settings.sidelen) as u16;
                let mut p = Pixel::new(x, y, (r, g, b), 0);
                let h = p.calc_heuristic(
                    (x, y),
                    target_pixels[i],
                    weights[i],
                    self.settings.proximity_importance,
                );
                p.update_heuristic(h);
                p
            })
            .collect::<Vec<_>>();

        let mut rng = Rand::with_seed(12345);
        let swaps_per_generation = SWAPS_PER_GENERATION_PER_PIXEL * pixels.len();

        let mut max_dist = self.settings.sidelen;
        loop {
            let mut swaps_made = 0;
            for _ in 0..swaps_per_generation {
                let apos = rng.gen_range(0..pixels.len() as u32) as usize;
                let ax = apos as u16 % self.settings.sidelen as u16;
                let ay = apos as u16 / self.settings.sidelen as u16;
                let bx = (ax as i16 + rng.gen_range(-(max_dist as i16)..(max_dist as i16 + 1)))
                    .clamp(0, self.settings.sidelen as i16 - 1) as u16;
                let by = (ay as i16 + rng.gen_range(-(max_dist as i16)..(max_dist as i16 + 1)))
                    .clamp(0, self.settings.sidelen as i16 - 1) as u16;
                let bpos = by as usize * self.settings.sidelen as usize + bx as usize;

                let t_a = target_pixels[apos];
                let t_b = target_pixels[bpos];

                let a_on_b_h = pixels[apos].calc_heuristic(
                    (bx, by),
                    t_b,
                    weights[bpos],
                    self.settings.proximity_importance,
                );

                let b_on_a_h = pixels[bpos].calc_heuristic(
                    (ax, ay),
                    t_a,
                    weights[apos],
                    self.settings.proximity_importance,
                );

                let improvement_a = pixels[apos].h - b_on_a_h;
                let improvement_b = pixels[bpos].h - a_on_b_h;
                if improvement_a + improvement_b > 0 {
                    // swap
                    pixels.swap(apos, bpos);
                    pixels[apos].update_heuristic(b_on_a_h);
                    pixels[bpos].update_heuristic(a_on_b_h);
                    swaps_made += 1;
                }
            }

            let assignments = pixels
                .iter()
                .map(|p| p.src_y as usize * self.settings.sidelen as usize + p.src_x as usize)
                .collect::<Vec<_>>();
            if max_dist < 4 && swaps_made < 10 {
                return Ok(Preset {
                    inner: UnprocessedPreset {
                        name: unprocessed.name,
                        width: self.settings.sidelen,
                        height: self.settings.sidelen,
                        source_img: source_pixels
                            .iter()
                            .flat_map(|(r, g, b)| [*r, *g, *b])
                            .collect(),
                    },
                    assignments,
                });
            }
            max_dist = (max_dist as f32 * 0.99).max(2.0) as u32;
        }
    }
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
