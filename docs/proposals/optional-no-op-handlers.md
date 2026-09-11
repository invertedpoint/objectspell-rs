# Proposal: let a receiver omit handlers it would leave empty

## Status

Proposed. Not scheduled. Needs to land in both implementations to keep them aligned.

## Problem

A receiver must define one method per signal its channel declares, and `connect()` rejects a
topology where one is missing. The rule is a good default: silence about a signal you receive is
usually an oversight, and finding it at startup beats finding it when the signal first fires.

But a component often genuinely cares about only part of a channel. Today it must still write
the other handlers as empty bodies. This repository's own `pubsub` example carries six of them:
`app::Connector::disconnected`, `app::Weather::weather_determined`,
`app::News::something_happened`, `connector::App::started`, `weather::App::stopped`, and
`news::App::stopped`. The shutdown and ordering tests add two more.

An empty body is indistinguishable from a handler someone meant to fill in. The rule that was
supposed to catch oversights ends up manufacturing things that look exactly like oversights.

## Proposed change

Let a receiver declare that it deliberately ignores the rest of its channel, and check only the
routes it did not opt out of. Two directions worth exploring:

1. **Opt out per receiver.** A marker on the receiver block meaning "the routes I do not name
   are deliberately ignored". Keeps the default strict; makes the exception explicit and
   greppable.
2. **Opt out per route.** Name the ignored routes explicitly, so adding a new signal to a channel
   still fails validation for every receiver that has not considered it. Stricter, more typing.

(1) is less ceremony; (2) preserves the property that a *new* signal is never silently ignored.
(2) is probably the right trade, since that property is most of the rule's value.

## Not in scope

Relaxing the check for a receiver that names a channel nothing emits. That is a different
mistake and should stay an error.

## Open questions

- Does the opt-out belong on the receiver block, or on the component?
- Should a receiver that opts out of *every* route be an error? It listens to a channel and
  handles nothing, which is almost certainly a mistake.
