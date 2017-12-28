trait BlockForge {
    fn create(block: &mut Block);
    fn validate(block: &Block) -> Option<Error>;
}