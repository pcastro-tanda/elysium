# Rule fixtures

Generated from RuboCop's own specs by `tools/port_spec.rb`; see the tool's
README for the format. Regenerate a directory with:

    ruby tools/port_spec.rb --cop Style/WordArray \
        --rubocop-src /path/to/rubocop-1.82.1 --out crates/rules/fixtures

Fixtures follow RuboCop 1.82.1 except where a later upstream bug fix was
adopted for corpus conformance; those directories are regenerated from that
later tag's checkout instead:

- `lint/number_conversion`: RuboCop 1.91.0 (1.88's #15252 safe-navigation
  message/no-autocorrect and #15194 `IgnoredClasses` -> `AllowedClasses`).

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
- `lint/redundant_cop_disable_directive/removes_cop_duplicated_by_department_and_leaves_free_text_as`
  and `lint/redundant_cop_disable_directive/removes_department_duplicated_by_department_and_leaves_free`:
  both remove a redundant single-cop directive embedded in a larger inline
  comment with trailing free text after the cop list (`# rubocop:disable
  Metrics/ClassLength - note`), expecting the removal to leave `# - note`
  behind. The rule's whole-comment removal only ever deletes the entire
  comment or nothing (documented in its `blind_spots`); preserving arbitrary
  trailing free text on a single-cop removal is a real, tracked gap, not an
  RSpec artifact.
- `lint/number_conversion/registers_an_offense_when_using_multiple_number_conversion_m`:
  `case foo.to_f ... end.to_i` -- confirmed against real RuboCop 1.82.1 to
  produce exactly two offenses, one of them a `case/when/else/end`
  expression interpolated into the message via `%<current>s`/
  `%<corrected_method>s`, so the message itself spans several physical
  lines. The fixture annotation format's `parse_annotation` only reads a
  message up to the end of its own physical line, so it cannot represent
  this: the reconstructed "de-annotated source" ends up with the message's
  continuation lines (`when 0.0`, `bar`, ...) spliced into the real source,
  which then fails to parse. Not a cop bug (verified directly against
  `rubocop --only Lint/NumberConversion`); a fixture-format limitation for
  the one case in this corpus where a message embeds a multi-line snippet.
- `lint/useless_assignment/registers_an_offense_21`, `..._22` and `..._23`
  keep their offense expectations but lost their `.fixed.rb`: the specs'
  expected corrections (`bar = do_something`, `foo = do_something`,
  `-bar = do_something`) are `expect_correction` artifacts. That helper
  reuses one cop object across autocorrection passes, so the byte ranges
  `IgnoredNode` collected for the chained assignment in pass 1 still
  suppress the offense in pass 2 and the loop stops early. Real RuboCop does
  not: `rubocop -A --only Lint/UselessAssignment` on all three snippets
  (verified against 1.91.0) converges to `do_something`, `do_something` and
  `-do_something`, which is what elysium produces. The remaining checks --
  offenses, and that the fix loop converges without leaving a correctable
  offense behind -- still run.
