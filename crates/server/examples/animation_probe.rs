//! Optional visual probe binary; leaves a running player_session executable intact.
#[path = "player_session.rs"]
mod fixture;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fixture::main()
}
