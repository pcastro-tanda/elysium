# Style/SpecialGlobalVars

Avoid Perl-style global variables.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Looks for uses of Perl-style global variables. Correcting to global
variables in the 'English' library will add a require statement to the top
of the file if enabled by the `RequireEnglish` config.

```ruby
# EnforcedStyle: use_english_names (default)

# good
require 'English' # or this could be in another file.

puts $LOAD_PATH
puts $LOADED_FEATURES
puts $PROGRAM_NAME
puts $ERROR_INFO
puts $ERROR_POSITION
puts $FIELD_SEPARATOR # or $FS
puts $OUTPUT_FIELD_SEPARATOR # or $OFS
puts $INPUT_RECORD_SEPARATOR # or $RS
puts $OUTPUT_RECORD_SEPARATOR # or $ORS
puts $INPUT_LINE_NUMBER # or $NR
puts $LAST_READ_LINE
puts $DEFAULT_OUTPUT
puts $DEFAULT_INPUT
puts $PROCESS_ID # or $PID
puts $CHILD_STATUS
puts $LAST_MATCH_INFO
puts $IGNORECASE
puts $ARGV # or ARGV
```

```ruby
# EnforcedStyle: use_perl_names

# good
puts $:
puts $"
puts $0
puts $!
puts $@
puts $;
puts $,
puts $/
puts $\
puts $.
puts $_
puts $>
puts $<
puts $$
puts $?
puts $~
puts $=
puts $*
```

```ruby
# EnforcedStyle: use_builtin_english_names

# good
# Like `use_perl_names` but allows builtin global vars.
puts $LOAD_PATH
puts $LOADED_FEATURES
puts $PROGRAM_NAME
puts ARGV
puts $:
puts $"
puts $0
puts $!
puts $@
puts $;
puts $,
puts $/
puts $\
puts $.
puts $_
puts $>
puts $<
puts $$
puts $?
puts $~
puts $=
puts $*
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `use_english_names` | `use_perl_names`, `use_english_names`, `use_builtin_english_names` | Which set of global variable names to enforce. |
| RequireEnglish | true |  | Whether autocorrection to an 'English' library name should also insert a `require 'English'` at the top of the file. |

## Blind spots

`config_to_allow_offenses` auto-config generation (the `--auto-gen-config`
result when Perl/mixed styles are used) is not ported: this engine has no
config-generation pass.
