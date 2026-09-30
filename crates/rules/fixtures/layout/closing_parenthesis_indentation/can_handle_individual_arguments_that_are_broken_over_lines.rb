corrector.insert_before(
  range,
  "
" + ' ' * (node.loc.keyword.column +
                indent_steps * configured_width)
)
