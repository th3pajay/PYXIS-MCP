use thiserror::Error;

use crate::{
    pyxis_client::PyxisClient,
    models::{Bin, BinStatus, Cert, Sku, User, Zone},
};

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("semantic: {0}")]
    Semantic(String),
    #[error("relational: {0}")]
    Relational(String),
    #[error("permissions: {0}")]
    Permissions(String),
    #[error(transparent)]
    Client(#[from] anyhow::Error),
}

pub async fn validate(bin: &Bin, sku: &Sku, user: &User, pyxis: &PyxisClient) -> Result<(), ValidationError> {
    if sku.weight_kg > bin.weight_cap_kg {
        return Err(ValidationError::Semantic(format!(
            "SKU weight {:.1}kg exceeds bin capacity {:.1}kg", sku.weight_kg, bin.weight_cap_kg
        )));
    }
    if sku.temp_zone != bin.zone {
        return Err(ValidationError::Semantic(format!(
            "SKU requires {:?} zone but bin is {:?}", sku.temp_zone, bin.zone
        )));
    }

    let live_bin = pyxis.get_bin(&bin.id).await?;
    if live_bin.status != BinStatus::Empty {
        return Err(ValidationError::Relational(format!(
            "bin {} is {:?}", bin.id, live_bin.status
        )));
    }

    let required = match bin.zone {
        Zone::Heavy => Cert::Forklift,
        Zone::Cold | Zone::Ambient => Cert::Picker,
    };
    if !user.certifications.contains(&required) {
        return Err(ValidationError::Permissions(format!(
            "user {} lacks {:?} certification", user.id, required
        )));
    }

    Ok(())
}
