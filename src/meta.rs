use serde::{Serialize, Deserialize, de::DeserializeOwned};
use super::crypto::*;
use super::header::*;
use super::signature::MetaSignature;

pub trait OtherMetadata
where Self: Serialize + DeserializeOwned + std::fmt::Debug + Default + Clone + Sized
{
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetaAuthorization
{
    allow_read: Vec<Hash>,
    allow_write: Vec<Hash>,
    implicit_authority: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetaTree
{
    pub parent: PrimaryKey,
    pub inherit_read: bool,
    pub inherit_write: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CoreMetadata
{
    None,
    Data(PrimaryKey),
    Tombstone(PrimaryKey),
    Authorization(MetaAuthorization),
    InitializationVector(InitializationVector),
    PublicKey(PublicKey),
    EncryptedPrivateKey(EncryptedPrivateKey),
    EncyptedEncryptionKey(EncryptKey),
    Tree(MetaTree),
    Signature(MetaSignature),
    Author(String),
}

impl Default for CoreMetadata {
    fn default() -> Self {
        CoreMetadata::None
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NoAdditionalMetadata { }
impl OtherMetadata for NoAdditionalMetadata { }

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct MetadataExt<M>
{
    pub core: Vec<CoreMetadata>,
    pub other: M,
}

#[allow(dead_code)]
pub type DefaultMetadata = MetadataExt<NoAdditionalMetadata>;