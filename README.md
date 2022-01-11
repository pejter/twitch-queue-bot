# Twitch queue bot

## Quickstart

### User commands

- !join -> Join the queue
  - The queue must be open to be able to join it
- !leave -> Leave the queue
- !position -> Display current queue position
- !length -> Display number of people in queue

### Mod commands

- !next -> Advance the queue by one and displays the new head of the queue
  - This will add the player to the player history which will make them unable to join again until a reset
- !list -> List the first 5 people in queue
- !clear -> Clear the queue
- !open -> Open the current queue for signups
- !close -> Close the current queue
- !reset -> Reset the player history
  - This **will not** clear the queue
- !create *name* -> Create a new queue with the rest of the message as its name
  - if the queue already exists this will overwrite it with a new one
- !select *name* -> Select the queue with a specific name (case insensitive)
  - queue must exist, use !create beforehand
  - only one queue may be selected at a time
- !save -> Save the queue to disk in it's current state
  - This won't close the queue!
  - The queues are persisted automatically unless the bot crashes or is forcibly killed.

## Features

- Queue management
- Persistence
- Player history
- Guaranteed order of message processing

## Hosting

## Limitation

- Single channel operation
