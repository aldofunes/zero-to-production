# TODO

- [x] What happens if a user tries to subscribe twice? Make sure that they receive two confirmation emails;
- [x] What happens if a user clicks on a confirmation link twice?
- [x] What happens if the subscription token is well-formatted but non-existent?
- [x] Add validation on the incoming token, we are currently passing the raw user input straight into a query (thanks sqlx for protecting us from SQL injections <3);
- [x] Use a proper templating solution for our emails (e.g. tera);
- [x] adding a n_retries and execute_after columns to keep track of how many attempts have already taken place and how long we should wait before
trying again. Try implementing it as an exercise!
- [x] If we experience a transient failure, we need to sleep for a while to improve our future chances of success.
This could be further refined by introducing an exponential backoff with [jitter](https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/).
- [ ] Expire idempotency keys