# mx Build Plan JSON Schema

The `mx compile --plan <file.json>` command accepts a JSON build plan that describes one or more compilation steps.

## Schema (version 1)

```json
{
  "version": 1,
  "steps": [
    {
      "entry": "src/Main.mod",
      "output": "target/myapp",
      "m2plus": false,
      "includes": ["src", "vendor/lib/src"],
      "extra_c": ["libs/sys.c"],
      "link_libs": ["m"],
      "link_paths": []
    }
  ]
}
```

## Fields

### Top-level

| Field     | Type    | Required | Description |
|-----------|---------|----------|-------------|
| `version` | integer | yes      | Must be `1` |
| `steps`   | array   | yes      | Ordered list of compilation steps |

### Step object

| Field        | Type     | Required | Default | Description |
|--------------|----------|----------|---------|-------------|
| `entry`      | string   | yes      | —       | Path to the main .mod file (relative to plan file) |
| `output`     | string   | no       | derived | Output binary path |
| `m2plus`     | boolean  | no       | false   | Enable Modula-2+ extensions |
| `includes`   | string[] | no       | []      | Additional include search paths (-I) |
| `extra_c`    | string[] | no       | []      | Extra .c files to compile and link |
| `link_libs`  | string[] | no       | []      | Libraries to link (-l flags) |
| `link_paths` | string[] | no       | []      | Library search paths (-L flags) |

All relative paths are resolved from the directory containing the plan file.

## Example

A plan that builds mxpkg itself:

```json
{
  "version": 1,
  "steps": [
    {
      "entry": "src/Main.mod",
      "output": "target/mxpkg",
      "includes": ["src"],
      "extra_c": ["../../libs/m2sys/m2sys.c"]
    }
  ]
}
```

## Usage

```sh
mx compile --plan build.json
```
