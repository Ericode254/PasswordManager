# PassTUI themes

Use a built-in preset or create a custom TOML theme. Both the main application
and `passtui --pick` use the selected theme, including backgrounds and dialogs.
Restart PassTUI after changing the configuration or a theme file; themes are
loaded once at startup.

## Built-in presets

- `catppuccin-mocha` (default)
- `catppuccin-latte`
- `tokyo-night`
- `gruvbox`
- `nord`
- `dracula`

Select one in `~/.config/passtui/config.toml`:

```toml
[theme]
name = "catppuccin-mocha"
```

On Linux, if `XDG_CONFIG_HOME` is set, the configuration directory is
`$XDG_CONFIG_HOME/passtui` instead of `~/.config/passtui`.

## Create your own theme

1. From the project directory, copy the provided example into your configuration
   directory. These commands work on Linux and honor `XDG_CONFIG_HOME`:

   ```sh
   mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/passtui/themes"
   cp themes/my-theme.toml "${XDG_CONFIG_HOME:-$HOME/.config}/passtui/themes/my-theme.toml"
   ```

   Choose a different filename if you already have `my-theme.toml` and want to
   keep it. Theme names use letters, numbers, hyphens, and underscores.

2. Edit that file. Start from a built-in preset and override as few or as many
   colors as you like:

   ```toml
   base = "catppuccin-mocha"

   [colors]
   primary = "#7DD3FC"
   background = "#101827"
   selection_bg = "#7DD3FC"
   selection_fg = "#101827"
   ```

   If `base` is omitted, it defaults to `catppuccin-mocha`. Each omitted color
   inherits from the base. The base must be a built-in preset, not another
   custom theme.

3. Edit the existing `[theme]` section in `config.toml`:

   ```toml
   [theme]
   name = "my-theme"
   ```

   Remove any existing `file` setting to select by name. The name resolves to
   `themes/my-theme.toml` inside the PassTUI configuration directory.
   Custom filenames are case-sensitive; built-in names take priority if a
   custom file has the same name as a preset.

4. Restart PassTUI. No recompilation or source-code changes are needed.

## Select an explicit file

You can also point to a file directly:

```toml
[theme]
file = "themes/my-theme.toml"
```

Relative paths are resolved against the **PassTUI configuration directory**,
not your shell's working directory. Absolute paths work too:

```toml
[theme]
file = "/home/alice/shared-themes/ocean.toml"
```

When both `name` and `file` are present, `file` takes priority. Paths are used
literally; shell variables and `~` are not expanded. Use a relative path or the
full absolute path.

## Available colors

| Key | Used for |
| --- | --- |
| `primary` | Active borders, active input, Git ahead indicator |
| `secondary` | Folder names |
| `accent` | Labels, search matches, popup borders, commit hashes |
| `text` | Normal text and values |
| `muted` | Hints, inactive borders/inputs, masked passwords |
| `background` | Main screen, picker, dialogs, and footer background |
| `selection_bg` | Selected row background |
| `selection_fg` | Selected row text |
| `success` | Success indicators, titles, strong password indicator |
| `warning` | Warning indicators, Git behind indicator |
| `error` | Errors, destructive confirmations, weak password indicator |

Each color accepts one of these formats:

```toml
[colors]
primary = "#7DD3FC"       # Six-digit RGB; uppercase/lowercase hex both work
accent = 208              # Terminal palette index, integer 0–255
background = "default"    # Use the terminal's default background
```

Use `"default"` for a foreground field to use the terminal's default foreground.
Hex colors require a terminal that supports true color for an exact match.
Choose contrasting text/background and selection colors so entries stay readable.

See [my-theme.toml](my-theme.toml) for a complete palette to copy and share.

## Troubleshooting

Missing files, invalid TOML, unknown color keys, invalid colors, and unknown
base themes produce a startup warning. PassTUI uses the complete default
`catppuccin-mocha` palette rather than a partially applied custom palette.
Other configuration settings remain in effect. If the interface has no colors at
all, check whether your environment sets `NO_COLOR`; PassTUI honors that terminal
preference.

Custom themes are local UI settings and do not change your password store or
Browserpass's appearance. To share a theme, share its TOML file; each user can
copy it into their own `passtui/themes` directory and select its name.
