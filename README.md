# License checker

`license-checker` is a binary to check or auto-insert a license header to each file.

```
license-checker --config example.toml lint test/main.cc
license-checker --config example.toml format test/main2.cc
```

Please see `example.toml`.

## Example

Original:
```
int main() { return 0; }
```

Formatted:
```
// Copyright (c) 2019 Presto Labs Pte. Ltd.
// Author: elbaro

int main() { return 0; }
```

The author is chosen by `git-blame HEAD` and counting LOC.
