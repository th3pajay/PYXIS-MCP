use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::models::{Bin, Sku};

pub struct VectorIndex {
    model: TextEmbedding,
    entries: Vec<(String, Vec<f32>)>,
    sku_entries: Vec<(String, Vec<f32>)>,
}

impl VectorIndex {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15))?;
        Ok(Self { model, entries: Vec::new(), sku_entries: Vec::new() })
    }

    pub fn index_bins(&mut self, bins: &[Bin]) -> Result<()> {
        let texts: Vec<String> = bins.iter()
            .map(|b| format!("Aisle {} Row {} Col {} zone {:?} capacity {}kg", b.aisle, b.row, b.col, b.zone, b.weight_cap_kg))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let vecs = self.model.embed(refs, None)?;
        self.entries = bins.iter().zip(vecs).map(|(b, v)| (b.id.clone(), v)).collect();
        Ok(())
    }

    pub fn index_skus(&mut self, skus: &[Sku]) -> Result<()> {
        let texts: Vec<String> = skus.iter()
            .map(|s| format!("{} zone {:?} weight {}kg", s.name, s.temp_zone, s.weight_kg))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let vecs = self.model.embed(refs, None)?;
        self.sku_entries = skus.iter().zip(vecs).map(|(s, v)| (s.id.clone(), v)).collect();
        Ok(())
    }

    pub fn search(&mut self, query: &str, k: usize) -> Result<Vec<(f32, String)>> {
        let q_vecs = self.model.embed(vec![query], None)?;
        let q = &q_vecs[0];
        let mut scores: Vec<(f32, String)> = self.entries.iter()
            .map(|(id, v)| (cosine_sim(q, v), id.clone()))
            .collect();
        scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores.into_iter().take(k).collect())
    }

    pub fn search_skus(&mut self, query: &str, k: usize) -> Result<Vec<(f32, String)>> {
        let q_vecs = self.model.embed(vec![query], None)?;
        let q = &q_vecs[0];
        let mut scores: Vec<(f32, String)> = self.sku_entries.iter()
            .map(|(id, v)| (cosine_sim(q, v), id.clone()))
            .collect();
        scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores.into_iter().take(k).collect())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
