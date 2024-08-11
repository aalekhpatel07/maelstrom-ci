# Maelstrom (Solution Framework w/ CI)

Use [Earthly](https://cloud.earthly.dev) to build the `ci` target including running the actual Maelstrom test:

```sh
earthly +ci \
    --maelstrom_args="test -w echo --bin /usr/local/bin/solutions --node-count 1 --time-limit 10"
```

This will generate reports locally to `runs/<commit_sha1>`, which can be accessed via a browser at [http://localhost:8000](http://localhost:8000) after serving with:

```sh
docker run --rm -p 8000:80 \
    maelstrom-with-rust-app:<commit_sha1>
```
