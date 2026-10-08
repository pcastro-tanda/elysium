# rubocop:push -Metrics/AbcSize -- special case
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `disable-next` instead of `push`/`pop` around a single statement.
def foo
  bar
end
# rubocop:pop
