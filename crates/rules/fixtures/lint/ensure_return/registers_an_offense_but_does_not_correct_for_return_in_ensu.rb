begin
  something
ensure
  file.close
  return
  ^^^^^^ Do not return from an `ensure` block.
end
