//! Stripe "expandable" fields: an ID by default, a full object when the
//! request asked for `expand[]`.

use serde::{Deserialize, Serialize};

/// A Stripe API object that has a typed identifier.
///
/// Implemented by every response object in the resource crates; it is what
/// allows [`Expandable`] to be generic and type-safe.
pub trait Object {
    /// The typed ID of this object (e.g. `CustomerId` for `Customer`).
    type Id: Clone + std::fmt::Debug;

    /// This object's identifier.
    fn id(&self) -> &Self::Id;
}

/// A field that Stripe returns as either a bare ID or an expanded object.
///
/// ```
/// use payrs_stripe_types::{Expandable, Object};
/// use payrs_stripe_types::ids::CustomerId;
///
/// #[derive(Debug, Clone, serde::Deserialize)]
/// struct Customer { id: CustomerId }
/// impl Object for Customer {
///     type Id = CustomerId;
///     fn id(&self) -> &CustomerId { &self.id }
/// }
///
/// let bare: Expandable<Customer> = serde_json::from_str("\"cus_123\"").unwrap();
/// assert_eq!(bare.id().as_str(), "cus_123");
/// assert!(bare.as_object().is_none());
///
/// let expanded: Expandable<Customer> =
///     serde_json::from_str(r#"{"id": "cus_123"}"#).unwrap();
/// assert!(expanded.as_object().is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Expandable<T: Object> {
    /// Only the object's ID was returned.
    Id(T::Id),
    /// The full object was returned (request used `expand[]`).
    Object(Box<T>),
}

impl<T: Object> Expandable<T> {
    /// The object's ID, whether or not the field was expanded.
    pub fn id(&self) -> &T::Id {
        match self {
            Self::Id(id) => id,
            Self::Object(obj) => obj.id(),
        }
    }

    /// The expanded object, if the field was expanded.
    pub fn as_object(&self) -> Option<&T> {
        match self {
            Self::Id(_) => None,
            Self::Object(obj) => Some(obj),
        }
    }

    /// Consume, returning the expanded object if present.
    pub fn into_object(self) -> Option<T> {
        match self {
            Self::Id(_) => None,
            Self::Object(obj) => Some(*obj),
        }
    }
}
