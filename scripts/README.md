# scripts/

Repository tooling, grouped by purpose. File names inside a folder are unchanged; Python folders have no
`__init__.py` and each is its own unittest discovery root (`discover -s scripts/<folder> -t scripts/<folder>`).

| Folder | Purpose | Run by | Tests |
|---|---|---|---|
| `release/` | `lib.sh`, the shared part of `platforms/<OS>/build/build.sh` (build farm, container choice, `dist/` cleanup) | `make build*` via the platform build scripts | `make test-dist` (`scripts/release/lib_test.sh`) |
| `release/dist/` | `publish.sh` and README templates: publishing a release to the distribution repository | release and Android workflows | `make test-dist` (`scripts/release/dist/publish_test.sh`) |
| `battery/` | the full acceptance battery, its report and the flaky-test log | `make battery` (and the tail of `make check`) | `make test-py` |
| `changed_tests/` | picks the tests affected by a change relative to `BASE` | `make test-changed` (`DRY=1` lists only) | `make test-py` |
| `checks/` | static checks of the tree without a build: core/platform boundary, page CSP | `make boundary` (`check_boundary.py`); `test_web_csp.py` is a test | `make test-py` |

One folder's Python tests: `python3 -m unittest discover -s scripts/<folder> -t scripts/<folder> -p 'test_*.py'`.
