# Rule fixtures

Generated from RuboCop's own specs by `tools/port_spec.rb`; see the tool's
README for the format. Regenerate a directory with:

    ruby tools/port_spec.rb --cop Style/WordArray \
        --rubocop-src /path/to/rubocop-1.82.1 --out crates/rules/fixtures

## Deliberately removed cases

Cases that depend on Ruby process state elysium does not model are deleted
after regeneration:

- `style/word_array/registers_an_offense_for_arrays_of_unicode_word_characters_2`:
  the spec sets `Encoding.default_external` to US-ASCII; elysium assumes UTF-8.
- `lint/debugger/does_not_register_an_offense_for_a_pry_debugger_call` and
  `..._2`: the spec disables the `Pry` group with an imperative
  `before { cur_cop_config['DebuggerMethods']['Pry'] = nil }` hook that
  `tools/port_spec.rb` cannot translate into a `.yml` override.
- `style/empty_else/autocorrect_missingelse_is_disabled_does_autocorrection` and
  `..._2` through `..._20` (20 cases): the spec builds a bare
  `RuboCop::Config.new` without merging `config/default.yml`, so
  `Style/MissingElse`'s `EnforcedStyle` is nil and autocorrection is never
  forbidden; under real CLI config loading (which elysium reproduces),
  `Enabled: true` resolves `EnforcedStyle: both` and RuboCop forbids the fix.
- `lint/duplicate_methods/adds_a_message_with_absolute_path` and
  `..._relative_path`: the offense message includes a path built by
  RuboCop's `smart_path`, which is relative to `Dir.pwd`; the two specs
  assert different formats for the same underlying path depending on the
  process's working directory.
- `lint/duplicate_methods/only_registers_an_offense_for_the_second_instance_of_a_dup_2`,
  `..._4`, `..._6`, `..._8`, and `..._du_10`: the spec reuses one cop
  instance across two separately-parsed source files, whereas RuboCop's CLI
  (and elysium) instantiates a fresh cop per file, so cross-file duplicate
  tracking never occurs in practice.
