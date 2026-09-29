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
- `style/hash_transform_keys`, `style/hash_transform_values`: RuboCop 1.91.0
  (`HashTransformMethod` switched from the `array_receiver?` blacklist to
  the `hash_receiver?` whitelist).
- `lint/interpolation_check`: RuboCop 1.91.0 (`valid_syntax?` requotes as
  `%{...}` when the source contains `"` and requires a `dstr` result).
- `lint/safe_navigation_with_empty`: RuboCop 1.91.0 (receiver pattern
  `(send ...)` became `!csend`).
- `lint/useless_method_definition`: RuboCop 1.91.0 (`**kwrest` joined the
  rest/optional exemption).
- `style/if_unless_modifier`: RuboCop 1.91.0 (skips nodes inside string
  interpolation, `MSG_USE_MODIFIER_PARENS` when the modifier form would need
  parens, and the `multiline_inside_collection?`/
  `another_modifier_if_on_same_line?` array/call/hash-literal guards).
- Phase 5 wave 5 (all ported from 1.91.0 source directly):
  `lint/assignment_in_condition`, `lint/deprecated_open_ssl_constant`,
  `lint/implicit_string_concatenation`, `lint/ineffective_access_modifier`,
  `lint/literal_as_condition`, `lint/literal_in_interpolation`,
  `lint/redundant_safe_navigation`, `lint/redundant_splat_expansion`,
  `lint/safe_navigation_consistency`, `lint/shadowed_argument`,
  `lint/underscore_prefixed_variable_name`, `lint/unused_block_argument`,
  `lint/unused_method_argument`, `lint/useless_setter_call`, `lint/void`,
  `naming/memoized_instance_variable_name`, `naming/predicate_prefix`,
  `naming/rescued_exceptions_variable_name`, `naming/variable_number`.
- Phase 5 wave 6 (all ported from 1.91.0 source directly):
  `style/percent_q_literals`, `style/end_block`, `style/symbol_literal`,
  `style/colon_method_definition`, `style/proc`, `style/when_then`,
  `style/array_join`, `style/negated_while`,
  `style/trailing_body_on_module`, `style/trailing_body_on_class`,
  `style/empty_lambda_parameter`, `style/multiline_if_then`,
  `style/variable_interpolation`, `style/strip`, `style/colon_method_call`,
  `style/def_with_parentheses`, `style/redundant_capital_w`,
  `style/empty_block_parameter`, `style/while_until_do`,
  `style/while_until_modifier`, `style/class_methods`, `style/class_check`,
  `style/trailing_body_on_method_definition`, `style/even_odd`,
  `style/unless_else`, `style/character_literal`, `style/stderr_puts`,
  `style/multiline_if_modifier`, `style/multiline_when_then`,
  `style/redundant_file_extension_in_require`,
  `style/trailing_method_end_statement`, `style/nested_ternary_operator`.
- Phase 5 wave 7 (all ported from 1.91.0 source directly):
  `style/min_max`, `style/single_argument_dig`,
  `style/preferred_hash_methods`, `style/bare_percent_literals`,
  `style/not`, `style/string_literals_in_interpolation`,
  `style/global_vars`, `style/global_std_stream`, `style/lambda_call`,
  `style/nested_parenthesized_calls`, `style/redundant_sort_by`,
  `style/stabby_lambda_parentheses`, `style/struct_inheritance`,
  `style/attr`, `style/keyword_parameters_order`,
  `style/redundant_conditional`, `style/redundant_exception`,
  `style/each_for_simple_loop`, `style/nil_comparison`,
  `style/negated_unless`, `style/for`, `style/or_assignment`,
  `style/self_assignment`, `style/multiline_memoization`,
  `style/negated_if`, `style/hash_as_last_array_item`,
  `style/nested_modifier`, `style/case_equality`,
  `style/redundant_self_assignment`, `style/redundant_percent_q`,
  `style/rescue_modifier`, `style/empty_method`.

A case that does not state `AllCops/TargetRubyVersion` in its `.yml` runs at
3.3: the specs were ported with `PARSER_ENGINE=parser_prism`, where RuboCop's
`CopHelper` resolves `let(:ruby_version)` to 3.3 and `tools/port_spec.rb`
therefore only records a version when the spec asked for a different one. The
harness (`crates/rules/tests/fixtures.rs`) injects it; the engine's own
default for real projects stays RuboCop's 2.7.

`tools/port_spec.rb` does not capture examples whose source is not valid
UTF-8: the two `with binary encoded source` cases each of
`lint/percent_string_array` and `lint/percent_symbol_array`.

## Deliberately removed cases

Cases that depend on Ruby process state elysium does not model are deleted
after regeneration:

- `style/unless_else/registers_offenses_for_both_but_corrects_only_the_outer_unle`:
  upstream keeps one cop instance across the correction loop, so
  `ignore_node` state from round 1 (stale offsets) still suppresses the inner
  `unless` in round 2; elysium starts every round with a fresh rule.
- `style/nested_parenthesized_calls/backslash_newline_in_method_call_registers_an_offense`
  and `style/redundant_conditional/registers_an_offense_for_if_elsif_else_with_boolean_results`
  / `..._with_negated_boolean`:
  the offense message contains a raw newline, which the one-line `^^^`
  annotation format cannot express.
- `style/lambda_call/style_is_set_to_call_registers_an_offense_for_x_3` and
  `..._4`, and `style/single_argument_dig/registers_and_corrects_an_offense`: like `style/unless_else` above, upstream relies on `ignore_node` state
  persisting across correction rounds, so the chained inner call stays
  uncorrected; elysium starts every round with a fresh rule.
- `style/rescue_modifier/excluded_file_processes_excluded_files_with_issue`: the
  case sets the cop's `Exclude`, which the fixture harness does not apply (the
  CLI does, before rules run).
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
- `bundler/gem_filename`: all 15 examples in the spec are un-portable by
  `tools/port_spec.rb`, so the directory holds zero fixture cases. The spec
  never calls `expect_offense`/`expect_no_offenses`/`expect_correction` --
  it stubs `processed_source.buffer.name` and asserts directly on
  `_investigate(cop, processed_source)`'s return value, which the tool's
  capture mechanism (monkeypatching those three helpers) cannot see.
  Verified instead with a throwaway harness invocation exercising every one
  of the spec's 15 `(EnforcedStyle, filename)` combinations against the
  ported rule directly (`RuleSet::only(&["Bundler/GemFilename"], &cfg)` +
  `linter::lint_parsed_with`), matching every expected message and the
  absence of an offense for every "valid gem file" case.
- `lint/script_permission/registers_an_offense_for_script_permission` and
  `lint/script_permission/if_autocorrection_is_off_leaves_the_file_intact`:
  the expected message embeds the basename of a random `Tempfile` created
  during the one-time upstream RSpec run; no deterministic port can
  reproduce it. The remaining cases cover the same logic.
- `layout/rescue_ensure_alignment/accepts_correctly_aligned_rescue_in_assigned_begin_end_block`:
  upstream's spec config has no `Layout/BeginEndAlignment` peer, while the
  fixture harness (like real RuboCop) merges `default.yml`, where
  `EnforcedStyleAlignWith: start_of_line` makes the case an offense; real
  RuboCop with its default configuration flags it too.
