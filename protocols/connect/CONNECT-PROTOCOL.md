# Aether Connect Protocol — Phase 11

Aether Connect defines native communication above Aether transport: identity-addressed conversations, presence, media references, groups and bounded offline delivery.

Messages carry conversation identity, sender node, sequence, media type, content reference and timestamp. Large media is represented through the native `ContentId` storage system rather than copied into control messages.

Offline queues are explicitly bounded and expire. Group membership is bounded. The current primitive deliberately does not claim end-to-end media encryption, push delivery, call negotiation, relay selection, or live L4 integration; those remain implementation and hardening work above this control model.
