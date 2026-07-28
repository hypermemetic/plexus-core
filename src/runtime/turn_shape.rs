//! `Turn<U, T, E>` — the one shape every method reduces to.
//!
//! # The distinction the type system can actually make
//!
//! An earlier design justified three return shapes by claiming `Stream` was
//! "potentially unbounded" and `Turn` was "bounded". That claim is not
//! enforceable: whether a stream terminates is undecidable in general, and a
//! `Turn` contains a stream, so it inherits exactly the same uncertainty.
//! A promise that does not come from the type system is documentation with
//! extra steps.
//!
//! So the only honest distinction is **what the author declared**:
//!
//! | declaration                | updates | terminal value |
//! |----------------------------|---------|----------------|
//! | `Result<T, E>`             | none    | `T`            |
//! | `impl Stream<Item = U>`    | `U`     | none           |
//! | `Turn<U, T, E>`            | `U`     | `T`            |
//!
//! All three are the *same* wire shape — updates\* then exactly one terminal
//! (RFC 002 §6.1). `Result` and `Stream` are sugar:
//!
//! ```text
//! Result<T, E>      ≡  Turn<Never, T, E>
//! Stream<Item = U>  ≡  Turn<U, (), E>
//! ```
//!
//! Keeping the sugar is a legibility decision, not a semantic one, and it is
//! worth stating that way: the return type tells a reader whether a method
//! emits and whether it answers, at a glance, without opening the body.
//!
//! # Why a closure and not a stream of items
//!
//! `Turn` could have been `Stream<Item = Either<U, T>>`. It is not, because
//! that shape cannot express "exactly one terminal, and it is last" — the
//! §6.1 invariant would become a runtime check. Here the body returns
//! `Result<T, E>` *once*, by construction, and the updates are a side channel.
//! The invariant is the function signature.

use futures::Stream;
use std::future::Future;
use std::pin::Pin;

type BoxFut<'a, O> = Pin<Box<dyn Future<Output = O> + Send + 'a>>;

/// The turn was cancelled: the peer is gone or a cancel signal arrived.
///
/// RFC 002 §6.8 requires cancellation be *delivered* to the turn rather than
/// merely recorded. Returning this from [`Emitter::emit`] is how it reaches a
/// body: `t.emit(x).await?` propagates it without the author writing a select.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the turn was cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// The update channel handed to a turn body.
///
/// Cloneable so a body can emit from concurrent tasks; every clone feeds the
/// same turn, and ordering between clones is the body's problem, not ours.
#[derive(Clone)]
pub struct Emitter<U> {
    tx: tokio::sync::mpsc::Sender<U>,
}

impl<U> Emitter<U> {
    /// Emit one update.
    ///
    /// `Err(Cancelled)` means stop: the receiver is gone. A body that
    /// propagates with `?` gets cooperative cancellation for free.
    pub async fn emit(&self, update: U) -> Result<(), Cancelled> {
        self.tx.send(update).await.map_err(|_| Cancelled)
    }

    /// Emit without waiting for capacity, dropping the update if the buffer is
    /// full. For progress that is worth sending but not worth blocking on.
    pub fn emit_lossy(&self, update: U) -> Result<(), Cancelled> {
        use tokio::sync::mpsc::error::TrySendError;
        match self.tx.try_send(update) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
            Err(TrySendError::Closed(_)) => Err(Cancelled),
        }
    }
}

/// A turn: zero or more `U` updates, then exactly one terminal — `T` or `E`.
///
/// Construct with [`Turn::new`] (emit and answer), [`Turn::from_stream`]
/// (forward, no answer), or [`Turn::answer`] (answer, no updates).
pub struct Turn<U, T, E> {
    body: Box<dyn FnOnce(Emitter<U>) -> BoxFut<'static, Result<T, E>> + Send>,
}

impl<U, T, E> Turn<U, T, E>
where
    U: Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    /// Emit, ask, and answer. The general form.
    ///
    /// ```ignore
    /// Turn::new(|t| async move {
    ///     t.emit(Progress::Resolving).await?;
    ///     Ok(Report { artifacts: 12 })
    /// })
    /// ```
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: FnOnce(Emitter<U>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, E>> + Send + 'static,
    {
        Turn { body: Box::new(move |e| Box::pin(f(e))) }
    }

    /// Answer once, emitting nothing. What `-> Result<T, E>` desugars to.
    pub fn answer(result: Result<T, E>) -> Self {
        Turn::new(move |_| async move { result })
    }

    /// Drive the turn.
    ///
    /// Returns the update stream and a future for the terminal. The caller —
    /// the generated handler — forwards updates as `<method>.update` frames and
    /// the terminal as the single `<method>.terminal` frame.
    ///
    /// Dropping the returned stream closes the channel, which surfaces in the
    /// body as [`Cancelled`] on its next `emit`.
    pub fn drive(self, buffer: usize) -> (impl Stream<Item = U> + Send, BoxFut<'static, Result<T, E>>) {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer.max(1));
        let fut = (self.body)(Emitter { tx });
        (tokio_stream::wrappers::ReceiverStream::new(rx), fut)
    }
}

impl<U, E> Turn<U, (), E>
where
    U: Send + 'static,
    E: Send + 'static,
{
    /// Forward a stream, answering with nothing. What `-> impl Stream` desugars
    /// to, and the short form for a method that only relays.
    ///
    /// ```ignore
    /// Turn::from_stream(self.llm.stream(prompt))
    /// ```
    pub fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item = U> + Send + 'static,
    {
        Turn::new(move |t| async move {
            futures::pin_mut!(stream);
            use futures::StreamExt;
            while let Some(item) = stream.next().await {
                t.emit(item).await.map_err(|_| ())
                    .map_or_else(|()| Err(()), Ok)
                    .ok();
            }
            Ok(())
        })
    }
}

impl<U, T, E> Turn<U, T, E>
where
    U: Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    /// Forward a stream and then answer, from a value computed over the run.
    ///
    /// ```ignore
    /// Turn::relay(self.llm.stream(p), |n| Summary { tokens: n })
    /// ```
    pub fn relay<S, F>(stream: S, ending: F) -> Self
    where
        S: Stream<Item = U> + Send + 'static,
        F: FnOnce(usize) -> T + Send + 'static,
    {
        Turn::new(move |t| async move {
            futures::pin_mut!(stream);
            use futures::StreamExt;
            let mut n = 0usize;
            while let Some(item) = stream.next().await {
                if t.emit(item).await.is_err() {
                    break;
                }
                n += 1;
            }
            Ok(ending(n))
        })
    }
}

/// What the macro emits for each declared return shape.
///
/// The point of this trait is that the handler table holds **one** thing. Three
/// declarations, one runtime path — the same "parse once, project many"
/// discipline applied to return types.
pub trait IntoTurn<U, T, E> {
    fn into_turn(self) -> Turn<U, T, E>;
}

impl<T, E> IntoTurn<std::convert::Infallible, T, E> for Result<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
{
    fn into_turn(self) -> Turn<std::convert::Infallible, T, E> {
        Turn::answer(self)
    }
}

impl<U, T, E> IntoTurn<U, T, E> for Turn<U, T, E> {
    fn into_turn(self) -> Turn<U, T, E> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[derive(Debug, PartialEq)]
    struct Boom;

    #[tokio::test]
    async fn a_result_emits_nothing_and_carries_a_value() {
        let (updates, fut) = Turn::<std::convert::Infallible, u8, Boom>::answer(Ok(7)).drive(4);
        let collected: Vec<_> = updates.collect().await;
        assert!(collected.is_empty(), "a Result declares no updates");
        assert_eq!(fut.await, Ok(7));
    }

    #[tokio::test]
    async fn a_turn_emits_then_answers_and_the_terminal_is_last() {
        let t = Turn::<u8, &'static str, Boom>::new(|e| async move {
            e.emit(1).await.unwrap();
            e.emit(2).await.unwrap();
            Ok("done")
        });
        let (updates, fut) = t.drive(4);
        let collected: Vec<_> = updates.collect().await;
        assert_eq!(collected, vec![1, 2]);
        assert_eq!(fut.await, Ok("done"));
    }

    #[tokio::test]
    async fn relay_forwards_and_answers_from_the_run() {
        let src = futures::stream::iter(vec!["a", "b", "c"]);
        let t = Turn::<&str, usize, Boom>::relay(src, |n| n);
        let (updates, fut) = t.drive(8);
        let collected: Vec<_> = updates.collect().await;
        assert_eq!(collected, vec!["a", "b", "c"]);
        assert_eq!(fut.await, Ok(3), "the terminal is computed over the run");
    }

    #[tokio::test]
    async fn dropping_the_updates_cancels_the_body() {
        let t = Turn::<u16, (), Boom>::new(|e| async move {
            // First emit may land in the buffer; the loop must observe the drop.
            for i in 0..1000u16 {
                if e.emit(i).await.is_err() {
                    return Err(Boom);
                }
            }
            Ok(())
        });
        let (updates, fut) = t.drive(1);
        drop(updates);
        assert_eq!(fut.await, Err(Boom), "cancellation reaches the body via emit");
    }
}
