def foo(a:, b:, c:,
        d:, e:, f:)
  do_something # rubocop:disable Metrics/ParameterLists
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unnecessary disabling of `Metrics/ParameterLists`.
end
