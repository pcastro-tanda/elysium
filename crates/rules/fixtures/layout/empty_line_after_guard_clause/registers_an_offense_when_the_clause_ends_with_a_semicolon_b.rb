def foo(item)
  return unless item.positive?;
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  item * 2
end
