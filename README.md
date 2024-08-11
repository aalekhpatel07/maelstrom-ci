# Maelstrom (Solution Framework w/ CI)

Use [Earthly](https://cloud.earthly.dev) to build the `ci` target:

```sh
earthly +ci
```

Then invoke the executable (in `/usr/local/bin/solutions`) under maelstrom's test env:
```sh
docker run --rm \
    maelstrom-with-rust-app:latest \
    test -w echo --bin "/usr/local/bin/solutions" --time-limit 5
```
