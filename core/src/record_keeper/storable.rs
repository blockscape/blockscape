use serde;

/// Storable objects are able to be stored in a `Database` instance.
/// Example implementation:
/// ```
/// #[derive(Serialize, Deserialize)]
/// struct Example {
///     a: u8,
///     b: u8
/// }
///
/// impl Storable for Example {
///     type DeserializeErr = String;
///     fn global_id() -> &'static [u8] { b"p" }
///     fn instance_id(&self) -> Vec<u8> { vec![self.a, self.b] }
/// }
/// ```
pub trait Storable: serde::Serialize + serde::de::DeserializeOwned {
    /// Error to be returned if it could not be deserialized correctly.
    // type DeserializeErr;

    /// Return a unique ID for the type, an example of this is b"plot", though the smallest
    /// reasonable values would be better, e.g. b"p" for plot. All storable types must return
    /// different IDs or there may be collisions.
    fn global_id() -> &'static [u8];

    /// Calculate and return a unique ID for the instance of this storable value. In the case of a
    /// plot, it would simply be the plot ID. It is for a block, then it would just be its Hash.
    /// This must not change between saves and loads for it to work correctly.
    fn instance_id(&self) -> Vec<u8>;

    /// Calculate and return the key-value of this object based on its global and instance IDs.
    fn key(&self) -> Vec<u8> {
        let mut key = Vec::new();
        key.extend_from_slice(Self::global_id());
        key.append(&mut self.instance_id());
        key
    }
}