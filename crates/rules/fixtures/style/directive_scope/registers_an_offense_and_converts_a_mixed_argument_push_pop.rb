# rubocop:disable Style/For
# rubocop:push -Metrics/AbcSize +Style/For
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `next` instead of `push`/`pop` around a single statement.
def foo
end
# rubocop:pop
# rubocop:enable Style/For
