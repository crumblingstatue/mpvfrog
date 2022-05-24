#[derive(PartialEq, Eq)]
pub enum PlaylistBehavior {
    Stop,
    Continue,
    RepeatOne,
    RepeatPlaylist,
}
