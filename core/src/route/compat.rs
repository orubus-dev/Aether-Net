use super::SignedRouteAdvertisement;

impl PartialEq for SignedRouteAdvertisement {
    fn eq(&self, other: &Self) -> bool {
        self.advertisement == other.advertisement
            && self.public_key == other.public_key
            && self.signature == other.signature
    }
}
impl Eq for SignedRouteAdvertisement {}
