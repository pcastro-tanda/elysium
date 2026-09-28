begin
  foo
ensure
  return baz
  ^^^^^^^^^^ Do not return from an `ensure` block.
end
