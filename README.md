Twitch queue bot
================

The twitch bot supports the following **user** commands:

- !join -> Join the queue
- !leave -> Leave the queue
- !position -> Display current queue position
- !length -> Display number of people in queue

and the following **mod** commands:

- !next -> Advance the queue by one and displays the new head of the queue
- !list -> List the first 5 people in queue
- !clear -> Clear the queue
- !create *name* -> Create a new queue with the rest of the message as the name
- !open *name* -> Open the queue with a specific name (case insensitive); queue must exist
- !close -> Close the current queue

Features
---------

- Queue management

TODO
-----

- [ ] Join/leave message batching
