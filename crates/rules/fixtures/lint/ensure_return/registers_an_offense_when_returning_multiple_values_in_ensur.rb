begin
  something
ensure
  do_something
  return foo, bar
  ^^^^^^^^^^^^^^^ Do not return from an `ensure` block.
end
