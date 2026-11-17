//! Controlled native NPC/player movement session; see common/npc.rs for scope.
#[allow(dead_code)]
#[path = "movement_session.rs"]
mod movement;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    movement::run(true).await
}
