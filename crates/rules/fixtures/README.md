# Rule fixtures

Generated from RuboCop 1.91.0's own specs (the version the corpus job pins in
`ci/corpus/*/Gemfile`) by `tools/port_spec.rb`; see the tool's README for the
format. Regenerate a directory with:

    ruby tools/port_spec.rb --cop Style/WordArray \
        --rubocop-src /path/to/rubocop-1.91.0 --out crates/rules/fixtures

`tools/check_fixtures.rb` (CI job "matches pinned RuboCop") regenerates
every directory from the pinned version and fails on any difference other
than the cases listed in `REMOVED.txt`, so fixtures can't lag the RuboCop
version the corpus is checked against. `--write` re-baselines them all.

A case that does not state `AllCops/TargetRubyVersion` in its `.yml` runs at
3.3: the specs were ported with `PARSER_ENGINE=parser_prism`, where RuboCop's
`CopHelper` resolves `let(:ruby_version)` to 3.3 and `tools/port_spec.rb`
therefore only records a version when the spec asked for a different one. The
harness (`crates/rules/tests/fixtures.rs`) injects it; the engine's own
default for real projects stays RuboCop's 2.7.

`tools/port_spec.rb` does not capture examples whose source is not valid
UTF-8: the two `with binary encoded source` cases each of
`lint/percent_string_array` and `lint/percent_symbol_array`.

Peer cops the rule under test reads (`config.for_cop('Layout/LineLength')['Max']`
and the like) are recorded per key while the spec runs. A key the spec's
bespoke `RuboCop::Config` leaves unset is nil there, while the harness merges
`default.yml`, so the `.yml` states such keys as `~` (a peer the spec never
mentions included).

## Deliberately removed cases

Cases that depend on Ruby process state elysium does not model are deleted
after regeneration. Each is listed in `REMOVED.txt` (one `dept/cop/case`
per line, which the check honours) and explained here:

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
- `style/percent_literal_delimiters`: 7 cases
  (`autocorrect_escape_characters_corrects_r_with_n_in_it`,
  `..._r_with_t_in_it`,
  `does_not_register_an_offense_for_other_delimiters_when_con_6`,
  `does_not_register_an_offense_for_preferred_delimiters_6`,
  `registers_an_offense_for_a_regular_expression_with_option`,
  `registers_an_offense_for_other_delimiters_4`,
  `registers_an_offense_for_other_delimiters_when_containing__4`): the spec's
  bare `PreferredDelimiters: {default: '[]'}` is not merged with `default.yml`;
  the real loader merges, so `'%r': '{}'` wins (same as the `style/empty_else`
  precedent).
- `style/if_inside_else`: `handles_a_nested_if_then_end`,
  `handles_a_nested_if_then_elsif_end`,
  `handles_a_nested_multiline_if_then_elsif_else_end`,
  `handles_a_deep_nested_multiline_if_then_elsif_else_end`: upstream
  `ignore_node` state persists across the spec's correction rounds; `rubocop -A`
  on the same snippets converges fully.
- `style/one_line_conditional`:
  `registers_and_corrects_an_offense_with_ternary_operator_fo_4` / `_5`:
  upstream `ignore_node` state persists across the spec's correction rounds
  (nested one-line `if` in the else branch).
- `style/parallel_assignment`:
  `registers_an_offense_when_a_lambda_with_parallel_assignment`: upstream
  `ignore_node` state persists across the spec's correction rounds (stale byte-
  offset coincidence).
- `style/if_with_semicolon`: 6 nested-if cases
  (`registers_an_offense_and_corrects_when_using_nested_if_end_i`,
  `..._nested_single_2`/`_3`/`_4`/`_l`,
  `registers_an_offense_when_a_nested_if_with_a_semicolon_is_us`): upstream
  `ignore_node` state persists across the spec's correction rounds.
- `style/parentheses_around_condition`:
  `does_not_accept_variable_assignment_in_condition_surrounded` /
  `does_not_accept_element_assignment_in_condition_surrounded_w`: the spec's
  isolated config has no `Lint/AssignmentInCondition` entry; the real loader
  supplies its `AllowSafeAssignment: true` default, which exempts the case
  (checked against `rubocop` 1.91.0 CLI).
- `style/block_delimiters`: `autocorrects_adjacent_curly_braces_correctly` /
  `registers_an_offense_for_nested_multi_line_blocks_with_trail`: upstream
  `ignore_node` state persists across the spec's correction rounds.
- `layout/space_around_block_parameters`: 5 lambda cases
  (`registers_an_offense_and_corrects_a_lambda_for_extra_space_b`,
  `..._multiple_spac`, `..._no_space_afte`, `..._no_space_befo`,
  `registers_an_offense_and_corrects_spacing_in_lambda_args`): the spec's
  isolated config leaves `Layout/SpaceInsideParens`'s `EnforcedStyle` unset.
  Real config loading merges default.yml's `no_space`, which conflicts with
  `EnforcedStyleInsidePipes: space` and suppresses the pipe/paren checks
  (checked against `rubocop` 1.91.0 CLI).
- `layout/empty_lines_around_access_modifier/registers_an_offense_and_partially_corrects`
  and `..._2` through `..._4`: with `Layout/EmptyLinesAroundBlockBody:
  no_empty_lines` and the modifier last in a block, upstream's
  `should_insert_line_after?` declines the fix, so a correctable offense
  survives; the spec checks one pass, the harness requires convergence.
- `metrics/block_length/ignoredmethods_is_enabled_regex_does_not_report_an_offense`:
  the spec passes a live Ruby `Regexp` in `IgnoredMethods`, which takes
  `AllowedMethods`' `Regexp` branch. A YAML config can only carry its
  `inspect` string, which real `rubocop` 1.91.0 treats as a plain method
  name, so the case can't be expressed as a config file.
- `lint/useless_assignment/registers_an_offense_21` through `_23`: chained
  assignments (`foo = bar = x`) whose expected one-round correction only
  arises because upstream reuses one cop instance across the spec's
  correction loop, so `ignore_node` ranges from round 1 still suppress round 2.
- `layout/indentation_width/registers_an_offense_for_bad_indentation_of_bodies`
  and `..._of_do_ensure_end_bl`: the expected correction relies on
  `@offense_ranges` from one correction round suppressing the next, because
  upstream reuses one cop instance across rounds. A fresh instance per round
  converges differently in the real `rubocop` 1.91.0 gem too.
- `lint/duplicate_methods`: `only_registers_an_offense_for_the_second_instance_of_a_du_12`
  and `_14`, `registers_an_offense_despite_silence_redefinition_of_metho_2`,
  `registers_an_offense_when_the_marker_names_a_different_met_2`,
  `registers_an_offense_when_the_other_definition_site_does_n_2`,
  `registers_an_offense_when_the_self_alias_trick_from_a_prev_2`: second
  files of 1.91.0's cross-file examples (project index,
  `AllowedCrossFilePaths`). Each offense depends on definitions recorded
  while linting another file in the same run; elysium lints files
  independently.
- `layout/extra_spacing/registers_offense_s_5` and `_10`: the expected
  correction only converges because upstream memoizes `@ignored_ranges` per
  cop instance and never resets it between correction rounds. The real
  `rubocop` 1.91.0 `-A` loop on the same source stops after one correction.
- `lint/redundant_cop_disable_directive/returns_no_offense_for_a_directive_on_a_later_line_of_the_of`:
  the spec injects a fake multi-line offense (`last_line: 2`), but
  `<case>.offenses` records only `Cop/Name:line`.
- `style/empty_else`, the 8 `AllowComments: true` cases (`registers_an_offense`,
  `_2`, `no_comment_and_empty_else_clause_registers_an_offense` and `_2`,
  `no_comment_and_nil_else_clause_registers_an_offense` and `_2`/`_3`,
  `not_comment_and_empty_else_clause_registers_an_offense`): same bare
  `RuboCop::Config` cause as the `autocorrect_missingelse_is_disabled` cases
  above. Real `rubocop` 1.91.0 reports these offenses but doesn't correct them.
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
