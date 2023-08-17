use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use actix::{ActorContext, Handler, Message};

/// Asks the server to stop itself.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Shutdown;

impl<G: GameArenaService> Handler<Shutdown> for Infrastructure<G> {
    type Result = ();

    fn handle(&mut self, _request: Shutdown, ctx: &mut Self::Context) -> Self::Result {
        ctx.stop();
    }
}
